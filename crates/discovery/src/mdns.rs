use crate::registry::{DiscoveredDevice, Registry};
use bolt_protocol::{DeviceKind, PROTOCOL_VERSION, SERVICE_TYPE};
use bolt_security::DeviceId;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::{
    collections::{BTreeSet, HashMap},
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::watch;
use tracing::{debug, warn};

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("mDNS error: {0}")]
    Mdns(#[from] mdns_sd::Error),
}

/// What we publish. Deliberately minimal: no paths, no user identity, no file info.
#[derive(Debug, Clone)]
pub struct Advertisement {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
    pub port: u16,
}

fn kind_str(k: DeviceKind) -> &'static str {
    match k {
        DeviceKind::Desktop => "desktop",
        DeviceKind::Phone => "phone",
        DeviceKind::Tablet => "tablet",
        DeviceKind::Unknown => "unknown",
    }
}

fn kind_parse(s: &str) -> DeviceKind {
    match s {
        "desktop" => DeviceKind::Desktop,
        "phone" => DeviceKind::Phone,
        "tablet" => DeviceKind::Tablet,
        _ => DeviceKind::Unknown,
    }
}

fn build_info(ad: &Advertisement) -> Result<ServiceInfo, DiscoveryError> {
    let instance = format!("bolt-{}", ad.id.short());
    let host = format!("{instance}.local.");
    let props: HashMap<String, String> = [
        ("v", PROTOCOL_VERSION.to_string()),
        ("id", ad.id.to_string()),
        ("n", ad.name.chars().take(60).collect()),
        ("k", kind_str(ad.kind).to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    Ok(ServiceInfo::new(SERVICE_TYPE, &instance, &host, "", ad.port, props)?.enable_addr_auto())
}

/// The set of usable local IPv4 addresses right now (loopback excluded).
fn local_ipv4s() -> BTreeSet<std::net::IpAddr> {
    if_addrs::get_if_addrs()
        .map(|v| {
            v.into_iter()
                .filter(|i| !i.is_loopback() && i.ip().is_ipv4())
                .map(|i| i.ip())
                .collect()
        })
        .unwrap_or_default()
}

/// How often the advertised addresses are compared with the machine's real ones.
const ADDR_WATCH_INTERVAL: Duration = Duration::from_secs(4);

/// Running advertise + browse session. Dropping or calling [`Discovery::stop`]
/// unregisters the service and stops multicast activity.
pub struct Discovery {
    daemon: ServiceDaemon,
    devices: watch::Receiver<Vec<DiscoveredDevice>>,
    /// Empty when this session only browses (nothing was registered).
    fullname: String,
    stopped: Arc<AtomicBool>,
}

impl Drop for Discovery {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

const DEVICE_TTL: Duration = Duration::from_secs(90);

/// How often the browse is restarted so a fresh query goes out. A device that opens later (or whose
/// one-shot announcement was lost to Wi-Fi power saving) is then found within this interval, and
/// devices that are still there keep refreshing their `last_seen` instead of expiring.
const BROWSE_REFRESH: Duration = Duration::from_secs(12);

impl Discovery {
    /// Advertise this device and browse for others.
    pub fn start(ad: &Advertisement) -> Result<Self, DiscoveryError> {
        Self::start_inner(ad.id.clone(), Some(ad))
    }

    /// Browse only: another component (the platform's own DNS-SD on Android) already advertises this device.
    /// Raw multicast on every interface also sees peers on networks the platform's browser skips, such as
    /// the phone's own hotspot, where the laptop is a client of the phone.
    pub fn browse(me: DeviceId) -> Result<Self, DiscoveryError> {
        Self::start_inner(me, None)
    }

    fn start_inner(me: DeviceId, ad: Option<&Advertisement>) -> Result<Self, DiscoveryError> {
        let daemon = ServiceDaemon::new()?;
        let mut fullname = String::new();
        if let Some(ad) = ad {
            let info = build_info(ad)?;
            fullname = info.get_fullname().to_string();
            daemon.register(info)?;
        }

        // The advertised addresses must follow the machine: joining Wi-Fi after the app started (or a
        // DHCP change) otherwise leaves peers resolving us to loopback/VM addresses they cannot reach.
        let stopped = Arc::new(AtomicBool::new(false));
        if let Some(ad) = ad {
            let (daemon, ad, stopped, fullname) = (
                daemon.clone(),
                ad.clone(),
                stopped.clone(),
                fullname.clone(),
            );
            tokio::spawn(async move {
                let mut known = local_ipv4s();
                loop {
                    tokio::time::sleep(ADDR_WATCH_INTERVAL).await;
                    if stopped.load(Ordering::Relaxed) {
                        break;
                    }
                    let now = local_ipv4s();
                    if now != known {
                        debug!(
                            ?known,
                            ?now,
                            "network addresses changed; re-registering mDNS service"
                        );
                        let _ = daemon.unregister(&fullname);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        match build_info(&ad).and_then(|i| daemon.register(i).map_err(Into::into)) {
                            Ok(()) => known = now,
                            Err(e) => warn!("mDNS re-register failed: {e}"),
                        }
                    }
                }
            });
        }

        let browse = daemon.browse(SERVICE_TYPE)?;
        let registry = Arc::new(Mutex::new(Registry::new(me, DEVICE_TTL)));
        let (tx, rx) = watch::channel(Vec::new());

        let reg = registry.clone();
        let tx2 = tx.clone();
        let browse_daemon = daemon.clone();
        tokio::spawn(async move {
            let mut browse = browse;
            let mut refresh = tokio::time::interval(BROWSE_REFRESH);
            refresh.tick().await; // the first tick is immediate
            loop {
                let ev = tokio::select! {
                    // recv_async errors when the daemon shuts down.
                    ev = browse.recv_async() => match ev {
                        Ok(ev) => ev,
                        Err(_) => break,
                    },
                    _ = refresh.tick() => {
                        let _ = browse_daemon.stop_browse(SERVICE_TYPE);
                        match browse_daemon.browse(SERVICE_TYPE) {
                            Ok(b) => browse = b,
                            Err(e) => {
                                warn!("mDNS browse restart failed: {e}");
                                break;
                            }
                        }
                        continue;
                    }
                };
                let mut r = reg.lock().unwrap_or_else(|p| p.into_inner());
                let changed = match ev {
                    ServiceEvent::ServiceResolved(info) => {
                        let source = info.get_fullname().to_string();
                        let (Some(id), Some(name)) = (
                            info.get_property_val_str("id")
                                .and_then(|s| DeviceId::parse(s).ok()),
                            info.get_property_val_str("n"),
                        ) else {
                            debug!(%source, "ignoring service without valid id/name");
                            continue;
                        };
                        if info.get_property_val_str("v") != Some(&PROTOCOL_VERSION.to_string()) {
                            continue;
                        }
                        let port = info.get_port();
                        let addrs: BTreeSet<SocketAddr> = info
                            .get_addresses()
                            .iter()
                            .map(|ip| SocketAddr::new(ip.to_ip_addr(), port))
                            .collect();
                        let kind = kind_parse(info.get_property_val_str("k").unwrap_or(""));
                        r.upsert(
                            &source,
                            DiscoveredDevice {
                                id,
                                name: name.chars().filter(|c| !c.is_control()).collect(),
                                kind,
                                addrs,
                            },
                            Instant::now(),
                        )
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => r.remove_source(&fullname),
                    _ => false,
                };
                let expired = r.expire(Instant::now()) > 0;
                if changed || expired {
                    let _ = tx2.send(r.snapshot());
                }
            }
        });

        // Periodic expiry so a vanished device (no goodbye) is eventually removed.
        let reg = registry;
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(15));
            loop {
                tick.tick().await;
                let mut r = reg.lock().unwrap_or_else(|p| p.into_inner());
                if r.expire(Instant::now()) > 0 && tx.send(r.snapshot()).is_err() {
                    break;
                }
                if tx.is_closed() {
                    break;
                }
            }
        });

        Ok(Self {
            daemon,
            devices: rx,
            fullname,
            stopped,
        })
    }

    /// Live view of nearby devices; updates only when the visible set changes.
    pub fn devices(&self) -> watch::Receiver<Vec<DiscoveredDevice>> {
        self.devices.clone()
    }

    /// Stop advertising and browsing (e.g. app backgrounded, no screen shown).
    pub fn stop(self) {
        self.stopped.store(true, Ordering::Relaxed);
        if !self.fullname.is_empty() {
            if let Err(e) = self.daemon.unregister(&self.fullname) {
                warn!("mDNS unregister failed: {e}");
            }
        }
        let _ = self.daemon.shutdown();
    }
}
