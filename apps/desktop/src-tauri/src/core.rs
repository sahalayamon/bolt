//! Application core: owns the Bolt engine objects and bridges their
//! channels (receiver events, progress watches, discovery) to Tauri events.
//! No networking is implemented here; everything goes through the crates.

use crate::dto::*;
use bolt_discovery::{Advertisement, DiscoveredDevice, Discovery, DEFAULT_PORT};
use bolt_protocol::DeviceKind;
use bolt_security::{DeviceId, Identity, TrustStore};
use bolt_transfer_core::*;
use bolt_transfer_store::{Direction, HistoryEntry, HistoryQuery, Status, Store};
use serde::Serialize;
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Clone)]
pub struct RetrySpec {
    pub paths: Vec<PathBuf>,
    pub addr: SocketAddr,
    pub peer: Option<DeviceId>,
}

struct TransferRec {
    dto: TransferDto,
    cancel: Option<CancellationToken>,
    retry: Option<RetrySpec>,
    /// Where received items ended up (top-level files/folders), for "show in folder".
    reveal: Vec<PathBuf>,
}

struct DiscoveryRun {
    handle: Discovery,
    cancel: CancellationToken,
}

pub struct CoreInner {
    pub app: AppHandle,
    pub identity: Arc<Identity>,
    pub store: Arc<Store>,
    pub ep: FlashEndpoint,
    pub receiver: ReceiverHandle,
    pub port: u16,
    discovery: Mutex<Option<DiscoveryRun>>,
    discovery_error: Mutex<Option<String>>,
    devices: Mutex<Vec<DiscoveredDevice>>,
    /// Devices found by dialling the default gateway directly (mDNS-independent).
    probed: Mutex<HashMap<DeviceId, DiscoveredDevice>>,
    pub settings: Mutex<SettingsDto>,
    offers: Mutex<HashMap<String, IncomingOffer>>,
    pairs: Mutex<HashMap<String, IncomingPair>>,
    offer_titles: Mutex<HashMap<String, String>>,
    /// Per offer: number of path components of each offered file (to find the top-level item it was saved under).
    offer_depths: Mutex<HashMap<String, Vec<usize>>>,
    transfers: Mutex<HashMap<String, TransferRec>>,
    counter: AtomicU64,
}

#[derive(Clone)]
pub struct Core(pub Arc<CoreInner>);

impl std::ops::Deref for Core {
    type Target = CoreInner;
    fn deref(&self) -> &CoreInner {
        &self.0
    }
}

pub fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Bolt Desktop".into())
}

fn local_ips() -> Vec<std::net::IpAddr> {
    if_addrs::get_if_addrs()
        .map(|v| v.into_iter().map(|i| i.ip()).collect())
        .unwrap_or_default()
}

/// True when every address of a discovered device is one of this computer's own.
pub(crate) fn is_local_only(
    addrs: &std::collections::BTreeSet<SocketAddr>,
    mine: &[std::net::IpAddr],
) -> bool {
    !addrs.is_empty()
        && addrs
            .iter()
            .all(|a| a.ip().is_loopback() || mine.contains(&a.ip()))
}

fn is_terminal_state(s: TransferState) -> bool {
    matches!(
        s,
        TransferState::Completed
            | TransferState::Failed
            | TransferState::Cancelled
            | TransferState::Rejected
    )
}

fn state_str(s: TransferState) -> &'static str {
    match s {
        TransferState::Connecting => "connecting",
        TransferState::AwaitingApproval => "awaiting_approval",
        TransferState::Transferring => "transferring",
        TransferState::Completed => "completed",
        TransferState::Failed => "failed",
        TransferState::Cancelled => "cancelled",
        TransferState::Rejected => "rejected",
    }
}

fn status_str(s: Status) -> &'static str {
    match s {
        Status::Completed => "completed",
        Status::Failed => "failed",
        Status::Cancelled => "cancelled",
        Status::Rejected => "rejected",
    }
}

fn blank_transfer(
    id: &str,
    direction: &'static str,
    peer_id: String,
    peer_name: String,
) -> TransferDto {
    TransferDto {
        id: id.to_string(),
        direction,
        peer_id,
        peer_name,
        state: if direction == "sent" {
            "connecting"
        } else {
            "transferring"
        },
        bytes_done: 0,
        bytes_total: 0,
        files_done: 0,
        files_total: 0,
        current_file: None,
        speed_bps: 0.0,
        eta_secs: None,
        error: None,
        title: String::new(),
        started_unix: now(),
        finished_unix: None,
        can_retry: false,
    }
}

impl Core {
    pub fn start(
        app: AppHandle,
        data_dir: PathBuf,
        default_download: PathBuf,
    ) -> Result<Core, String> {
        std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
        let identity = Arc::new(Identity::load_or_create(&data_dir).map_err(|e| e.to_string())?);
        let store =
            Arc::new(Store::open(&data_dir.join("bolt.sqlite")).map_err(|e| e.to_string())?);
        let settings = load_settings(&store, &default_download);
        let want_port: u16 = std::env::var("BOLT_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        let ep = FlashEndpoint::bind(
            identity.clone(),
            SocketAddr::from(([0, 0, 0, 0], want_port)),
        )
        .or_else(|_| FlashEndpoint::bind(identity.clone(), SocketAddr::from(([0, 0, 0, 0], 0))))
        .map_err(|e| e.to_string())?;
        let port = ep.local_addr().map_err(|e| e.to_string())?.port();
        let cfg = ReceiverConfig {
            download_dir: PathBuf::from(&settings.download_dir),
            local: LocalInfo {
                name: settings.device_name.clone(),
                kind: DeviceKind::Desktop,
            },
            auto_accept_trusted: settings.auto_accept_trusted,
            max_concurrent_transfers: 3,
            transfer: TransferConfig::default(),
        };
        let (receiver, events) = Receiver::start(ep.clone(), cfg, store.clone());
        let core = Core(Arc::new(CoreInner {
            app,
            identity,
            store,
            ep,
            receiver,
            port,
            discovery: Mutex::new(None),
            discovery_error: Mutex::new(None),
            devices: Mutex::new(Vec::new()),
            probed: Mutex::new(HashMap::new()),
            settings: Mutex::new(settings),
            offers: Mutex::new(HashMap::new()),
            pairs: Mutex::new(HashMap::new()),
            offer_titles: Mutex::new(HashMap::new()),
            offer_depths: Mutex::new(HashMap::new()),
            transfers: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
        }));
        core.start_discovery();
        let c = core.clone();
        tauri::async_runtime::spawn(async move { c.probe_gateway_loop().await });
        let c = core.clone();
        tauri::async_runtime::spawn(async move { c.discovery_watchdog().await });
        let c = core.clone();
        tauri::async_runtime::spawn(async move { c.context_menu_loop().await });
        let c = core.clone();
        tauri::async_runtime::spawn(async move { c.event_loop(events).await });
        Ok(core)
    }

    pub fn emit<S: Serialize + Clone>(&self, name: &str, payload: S) {
        let _ = self.app.emit(name, payload);
    }

    pub fn new_id(&self, prefix: &str) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{prefix}-{t:x}-{n}")
    }

    pub fn local_info(&self) -> LocalInfo {
        LocalInfo {
            name: lock(&self.settings).device_name.clone(),
            kind: DeviceKind::Desktop,
        }
    }

    // ---- discovery ----------------------------------------------------

    pub fn start_discovery(&self) {
        let name = lock(&self.settings).device_name.clone();
        if let Some(old) = lock(&self.discovery).take() {
            old.cancel.cancel();
            old.handle.stop();
        }
        lock(&self.devices).clear();
        self.emit("devices-changed", self.devices_dto());
        let ad = Advertisement {
            id: self.identity.device_id().clone(),
            name,
            kind: DeviceKind::Desktop,
            port: self.port,
        };
        match Discovery::start(&ad) {
            Ok(handle) => {
                *lock(&self.discovery_error) = None;
                let cancel = CancellationToken::new();
                let mut rx = handle.devices();
                let core = self.clone();
                let c2 = cancel.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = c2.cancelled() => break,
                            r = rx.changed() => if r.is_err() { break },
                        }
                        let list = rx.borrow_and_update().clone();
                        *lock(&core.devices) = list;
                        core.emit("devices-changed", core.devices_dto());
                    }
                });
                *lock(&self.discovery) = Some(DiscoveryRun { handle, cancel });
            }
            Err(e) => {
                *lock(&self.discovery_error) = Some(e.to_string());
            }
        }
    }

    /// If nothing has been heard over mDNS for a while, rebuild the mDNS session (the multicast
    /// socket can go stale after sleep, a Wi-Fi/hotspot change or a firewall prompt) instead of
    /// making the user restart the app.
    async fn discovery_watchdog(&self) {
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        tick.tick().await;
        loop {
            tick.tick().await;
            let idle = lock(&self.devices).is_empty();
            let failed = lock(&self.discovery_error).is_some() || lock(&self.discovery).is_none();
            if idle || failed {
                self.start_discovery();
            }
        }
    }

    /// mDNS results plus gateway-probed devices (mDNS wins when both know a device).
    fn all_devices(&self) -> Vec<DiscoveredDevice> {
        let mut v = lock(&self.devices).clone();
        for d in lock(&self.probed).values() {
            if !v.iter().any(|x| x.id == d.id) {
                v.push(d.clone());
            }
        }
        // Anything that only lives on this computer (another copy of the app, a leftover process)
        // is not somewhere you can send files to. BOLT_SHOW_LOCAL_DEVICES=1 keeps them for tests.
        if std::env::var_os("BOLT_SHOW_LOCAL_DEVICES").is_none() {
            let mine = local_ips();
            v.retain(|d| !is_local_only(&d.addrs, &mine));
        }
        v
    }

    /// Every ~10 s, dial the default gateway on our port. When this computer is on a phone's
    /// hotspot the gateway is the phone, and Android does not answer mDNS on that link.
    async fn probe_gateway_loop(&self) {
        let cfg = TransferConfig {
            connect_timeout: Duration::from_millis(2500),
            handshake_timeout: Duration::from_millis(2500),
            ..TransferConfig::default()
        };
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        let mut n: u32 = 0;
        loop {
            tick.tick().await;
            n = n.wrapping_add(1);
            // Hidden in the tray: probe a third as often (saves wake-ups and battery).
            if self.window_hidden() && n % 3 != 0 {
                continue;
            }
            let gws = tokio::task::spawn_blocking(crate::netinfo::default_gateways)
                .await
                .unwrap_or_default();
            let mut found: HashMap<DeviceId, DiscoveredDevice> = HashMap::new();
            for gw in gws.into_iter().take(3) {
                let addr = SocketAddr::from((gw, DEFAULT_PORT));
                if let Ok(peer) = probe_peer(&self.ep, addr, &self.local_info(), &cfg).await {
                    if peer.id != *self.identity.device_id() {
                        found.insert(
                            peer.id.clone(),
                            DiscoveredDevice {
                                id: peer.id,
                                name: peer.name.chars().filter(|c| !c.is_control()).collect(),
                                kind: peer.kind,
                                addrs: [addr].into_iter().collect(),
                            },
                        );
                    }
                }
            }
            let changed = {
                let mut p = lock(&self.probed);
                let same = p.len() == found.len()
                    && found.iter().all(|(k, v)| p.get(k).is_some_and(|o| o == v));
                if !same {
                    *p = found;
                }
                !same
            };
            if changed {
                self.emit("devices-changed", self.devices_dto());
            }
        }
    }

    pub fn devices_dto(&self) -> Vec<DeviceDto> {
        self.all_devices()
            .iter()
            .map(|d| DeviceDto::from_discovered(d, self.store.is_trusted(&d.id)))
            .collect()
    }

    pub fn discovery_state(&self) -> (bool, Option<String>) {
        (
            lock(&self.discovery).is_some(),
            lock(&self.discovery_error).clone(),
        )
    }

    /// Prefer a routable IPv4 address; IPv6 link-local addresses cannot be dialled without a scope id.
    pub fn address_for(&self, id: &DeviceId) -> Option<SocketAddr> {
        let devices = self.all_devices();
        let d = devices.iter().find(|d| &d.id == id)?;
        // A peer on this very computer is reached over loopback (dialling our own LAN address
        // from the same socket is unreliable, e.g. on some hotspot adapters).
        if is_local_only(&d.addrs, &local_ips()) {
            if let Some(lo) = d.addrs.iter().find(|a| a.ip().is_loopback() && a.is_ipv4()) {
                return Some(*lo);
            }
        }
        let rank = |a: &&SocketAddr| match a {
            SocketAddr::V4(v) if v.ip().is_loopback() => 3,
            SocketAddr::V4(v) if v.ip().is_link_local() => 2,
            SocketAddr::V4(_) => 0,
            SocketAddr::V6(_) => 4,
        };
        d.addrs.iter().min_by_key(rank).copied()
    }

    pub fn device_name_of(&self, id: &DeviceId) -> Option<String> {
        self.all_devices()
            .iter()
            .find(|d| &d.id == id)
            .map(|d| d.name.clone())
            .or_else(|| {
                self.store
                    .list()
                    .ok()?
                    .into_iter()
                    .find(|t| &t.id == id)
                    .map(|t| t.name)
            })
    }

    pub fn online_ids(&self) -> Vec<DeviceId> {
        self.all_devices().iter().map(|d| d.id.clone()).collect()
    }

    // ---- transfers ----------------------------------------------------

    pub fn transfers_snapshot(&self) -> Vec<TransferDto> {
        let mut v: Vec<_> = lock(&self.transfers)
            .values()
            .map(|r| r.dto.clone())
            .collect();
        v.sort_by_key(|t| std::cmp::Reverse(t.started_unix));
        v
    }

    fn update_transfer(&self, id: &str, f: impl FnOnce(&mut TransferDto)) {
        let snap = {
            let mut t = lock(&self.transfers);
            let Some(rec) = t.get_mut(id) else { return };
            f(&mut rec.dto);
            rec.dto.clone()
        };
        self.emit("transfer-update", snap);
    }

    fn forward_progress(&self, id: String, mut rx: watch::Receiver<Progress>) {
        let core = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut meter = SpeedMeter::default();
            while rx.changed().await.is_ok() {
                let p = rx.borrow_and_update().clone();
                let speed = meter.sample(Instant::now(), p.bytes_done);
                let eta = meter
                    .eta(p.bytes_total.saturating_sub(p.bytes_done))
                    .map(|d| d.as_secs_f64());
                core.update_transfer(&id, |d| {
                    // Terminal state comes from the authoritative result, not the watch.
                    if d.is_terminal() {
                        return;
                    }
                    if !is_terminal_state(p.state) {
                        d.state = state_str(p.state);
                    }
                    d.bytes_done = p.bytes_done;
                    if p.bytes_total > 0 {
                        d.bytes_total = p.bytes_total;
                    }
                    d.files_done = p.files_done;
                    if p.files_total > 0 {
                        d.files_total = p.files_total;
                    }
                    if let Some(f) = &p.current_file {
                        d.current_file = Some(f.clone());
                        if d.title.is_empty() {
                            d.title = f.clone();
                        }
                    }
                    d.speed_bps = speed;
                    d.eta_secs = eta;
                });
            }
        });
    }

    fn record_history(&self, d: &TransferDto) {
        let status = match d.state {
            "completed" => Status::Completed,
            "cancelled" => Status::Cancelled,
            "rejected" => Status::Rejected,
            _ => Status::Failed,
        };
        let entry = HistoryEntry {
            transfer_id: d.id.clone(),
            direction: if d.direction == "sent" {
                Direction::Sent
            } else {
                Direction::Received
            },
            peer_id: if d.peer_id.is_empty() {
                "unknown".into()
            } else {
                d.peer_id.clone()
            },
            peer_name: d.peer_name.clone(),
            status,
            file_count: d.files_total,
            title: d.title.clone(),
            total_bytes: d.bytes_total,
            started_unix: d.started_unix,
            finished_unix: d.finished_unix.unwrap_or_else(now),
            error: d.error.clone(),
        };
        if let Err(e) = self.store.record_transfer(&entry) {
            tracing::error!("recording history failed: {e}");
        }
        self.emit("history-changed", ());
    }

    /// Create (or reset, for retry) a sent-transfer record and run it in the background.
    pub fn spawn_send(
        &self,
        id: Option<String>,
        spec: RetrySpec,
        peer_name: String,
    ) -> TransferDto {
        let id = id.unwrap_or_else(|| self.new_id("s"));
        let dto = blank_transfer(
            &id,
            "sent",
            spec.peer
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default(),
            peer_name,
        );
        let cancel = CancellationToken::new();
        lock(&self.transfers).insert(
            id.clone(),
            TransferRec {
                dto: dto.clone(),
                cancel: Some(cancel.clone()),
                retry: Some(spec.clone()),
                reveal: Vec::new(),
            },
        );
        self.emit("transfer-update", dto.clone());
        let core = self.clone();
        tauri::async_runtime::spawn(async move { core.run_send(id, spec, cancel).await });
        dto
    }

    async fn run_send(&self, id: String, spec: RetrySpec, cancel: CancellationToken) {
        let paths = spec.paths.clone();
        let scanned = tokio::task::spawn_blocking(move || scan(&paths)).await;
        let items = match scanned {
            Ok(Ok(items)) if !items.is_empty() => items,
            Ok(Ok(_)) => return self.fail_send(&id, "Nothing to send: no files found".into()),
            Ok(Err(e)) => return self.fail_send(&id, format!("Cannot read selection: {e}")),
            Err(e) => return self.fail_send(&id, e.to_string()),
        };
        let total: u64 = items.iter().map(|i| i.size).sum();
        let count = items.len() as u32;
        let first = items
            .first()
            .map(|i| i.rel_path.split('/').next().unwrap_or("").to_string())
            .unwrap_or_default();
        self.update_transfer(&id, |d| {
            d.bytes_total = total;
            d.files_total = count;
            d.title = first;
        });
        let (reporter, rx) = ProgressReporter::new();
        self.forward_progress(id.clone(), rx);
        let req = SendRequest {
            addr: spec.addr,
            expected_peer: spec.peer.clone(),
            local: self.local_info(),
            items,
            config: TransferConfig::default(),
        };
        let result = send_files(&self.ep, req, cancel, reporter).await;
        // Let the progress forwarder drain before the authoritative state lands.
        tokio::time::sleep(Duration::from_millis(40)).await;
        let snap = {
            let mut t = lock(&self.transfers);
            let Some(rec) = t.get_mut(&id) else { return };
            let d = &mut rec.dto;
            d.finished_unix = Some(now());
            d.speed_bps = 0.0;
            d.eta_secs = None;
            d.current_file = None;
            match &result {
                Ok(s) => {
                    d.state = "completed";
                    d.bytes_done = s.bytes;
                    d.bytes_total = s.bytes;
                    d.files_done = s.files;
                    d.peer_id = s.peer.id.to_string();
                    d.peer_name = s.peer.name.clone();
                    rec.retry = None;
                }
                Err(e) => {
                    d.state = match e {
                        TransferError::Rejected(_) => "rejected",
                        e if e.is_cancel() => "cancelled",
                        _ => "failed",
                    };
                    d.error = Some(e.to_string());
                    d.can_retry = d.state != "rejected";
                }
            }
            rec.cancel = None;
            rec.dto.clone()
        };
        self.record_history(&snap);
        self.emit("transfer-update", snap);
    }

    fn fail_send(&self, id: &str, msg: String) {
        self.update_transfer(id, |d| {
            d.state = "failed";
            d.error = Some(msg);
            d.finished_unix = Some(now());
            d.can_retry = true;
        });
        let snap = lock(&self.transfers).get(id).map(|r| r.dto.clone());
        if let Some(d) = snap {
            self.record_history(&d);
        }
    }

    /// Items to highlight for "show in folder" (empty if unknown).
    pub fn reveal_paths(&self, id: &str) -> Vec<PathBuf> {
        lock(&self.transfers)
            .get(id)
            .map(|r| r.reveal.clone())
            .unwrap_or_default()
    }

    pub fn cancel_transfer(&self, id: &str) -> Result<(), String> {
        let t = lock(&self.transfers);
        match t.get(id).and_then(|r| r.cancel.clone()) {
            Some(c) => {
                c.cancel();
                Ok(())
            }
            None => Err("This transfer is not running".into()),
        }
    }

    pub fn retry_transfer(&self, id: &str) -> Result<TransferDto, String> {
        let (spec, name) = {
            let t = lock(&self.transfers);
            let rec = t.get(id).ok_or("Unknown transfer")?;
            if !rec.dto.is_terminal() || rec.dto.state == "completed" {
                return Err("Only failed or cancelled transfers can be retried".into());
            }
            (
                rec.retry.clone().ok_or("This transfer cannot be retried")?,
                rec.dto.peer_name.clone(),
            )
        };
        Ok(self.spawn_send(Some(id.to_string()), spec, name))
    }

    pub fn dismiss_transfer(&self, id: &str) {
        let mut t = lock(&self.transfers);
        if t.get(id).is_some_and(|r| r.dto.is_terminal()) {
            t.remove(id);
        }
    }

    // ---- incoming -----------------------------------------------------

    pub fn respond_offer(
        &self,
        id: &str,
        accept: bool,
        dest: Option<String>,
    ) -> Result<(), String> {
        let offer = lock(&self.offers)
            .remove(id)
            .ok_or("This offer is no longer available (it may have timed out)")?;
        if !accept {
            offer.reject("declined by user");
            return Ok(());
        }
        let first = offer
            .offer
            .files
            .first()
            .map(|f| f.path.split('/').next().unwrap_or("").to_string())
            .unwrap_or_default();
        lock(&self.offer_titles).insert(id.to_string(), first);
        lock(&self.offer_depths).insert(
            id.to_string(),
            offer
                .offer
                .files
                .iter()
                .map(|f| f.path.split('/').count())
                .collect(),
        );
        match dest.filter(|d| !d.is_empty()) {
            Some(d) => offer.accept_into(PathBuf::from(d)),
            None => offer.accept(),
        }
        Ok(())
    }

    pub fn respond_pair(&self, id: &str, approve: bool) -> Result<(), String> {
        let p = lock(&self.pairs)
            .remove(id)
            .ok_or("This pairing request is no longer available")?;
        if approve {
            p.approve();
            let core = self.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                core.emit("trusted-changed", ());
            });
        } else {
            p.deny();
        }
        Ok(())
    }

    async fn event_loop(&self, mut events: tokio::sync::mpsc::Receiver<IncomingEvent>) {
        while let Some(ev) = events.recv().await {
            match ev {
                IncomingEvent::OfferReceived(o) => {
                    let id = o.offer.transfer_id.clone();
                    let dto = OfferDto {
                        id: id.clone(),
                        peer_id: o.peer.id.to_string(),
                        peer_short_id: o.peer.id.short().to_string(),
                        peer_name: o.peer.name.clone(),
                        peer_kind: kind_str(o.peer.kind),
                        trusted: o.trusted,
                        total_bytes: o.total_bytes,
                        free_space: o.free_space,
                        file_count: o.offer.files.len(),
                        files: o
                            .offer
                            .files
                            .iter()
                            .take(200)
                            .map(|f| OfferFileDto {
                                path: f.path.clone(),
                                size: f.size,
                            })
                            .collect(),
                        download_dir: lock(&self.settings).download_dir.clone(),
                    };
                    lock(&self.offers).insert(id, o);
                    self.attention();
                    self.emit("incoming-offer", dto);
                }
                IncomingEvent::PairRequested(p) => {
                    let id = self.new_id("p");
                    let dto = PairRequestDto {
                        id: id.clone(),
                        peer_id: p.peer.id.to_string(),
                        peer_short_id: p.peer.id.short().to_string(),
                        peer_name: p.peer.name.clone(),
                        code: p.code.clone(),
                    };
                    lock(&self.pairs).insert(id, p);
                    self.attention();
                    self.emit("incoming-pair", dto);
                }
                IncomingEvent::TransferStarted {
                    transfer_id,
                    peer,
                    progress,
                    cancel,
                } => {
                    let title = lock(&self.offer_titles)
                        .remove(&transfer_id)
                        .unwrap_or_default();
                    let mut dto = blank_transfer(
                        &transfer_id,
                        "received",
                        peer.id.to_string(),
                        peer.name.clone(),
                    );
                    dto.title = title;
                    lock(&self.transfers).insert(
                        transfer_id.clone(),
                        TransferRec {
                            dto: dto.clone(),
                            cancel: Some(cancel),
                            retry: None,
                            reveal: Vec::new(),
                        },
                    );
                    self.emit("transfer-update", dto);
                    self.forward_progress(transfer_id, progress);
                }
                IncomingEvent::TransferFinished {
                    transfer_id,
                    peer,
                    result,
                } => {
                    // Let the progress forwarder flush its last sample first.
                    tokio::time::sleep(Duration::from_millis(40)).await;
                    let snap = {
                        let mut t = lock(&self.transfers);
                        let rec = t.entry(transfer_id.clone()).or_insert_with(|| TransferRec {
                            dto: blank_transfer(
                                &transfer_id,
                                "received",
                                peer.id.to_string(),
                                peer.name.clone(),
                            ),
                            cancel: None,
                            retry: None,
                            reveal: Vec::new(),
                        });
                        let mut reveal_items: Vec<PathBuf> = Vec::new();
                        let d = &mut rec.dto;
                        d.finished_unix = Some(now());
                        d.speed_bps = 0.0;
                        d.eta_secs = None;
                        d.current_file = None;
                        match &result {
                            Ok(s) => {
                                d.state = "completed";
                                d.bytes_done = s.bytes;
                                d.bytes_total = s.bytes;
                                d.files_done = s.files;
                                d.files_total = s.files;
                                let depths = lock(&self.offer_depths)
                                    .remove(&transfer_id)
                                    .unwrap_or_default();
                                let mut items: Vec<PathBuf> = Vec::new();
                                for (i, p) in s.saved_paths.iter().enumerate() {
                                    let depth = depths.get(i).copied().unwrap_or(1).max(1);
                                    let top =
                                        p.ancestors().nth(depth - 1).unwrap_or(p).to_path_buf();
                                    if !items.contains(&top) {
                                        items.push(top);
                                    }
                                }
                                reveal_items = items;
                                if d.title.is_empty() {
                                    d.title = s
                                        .saved_paths
                                        .first()
                                        .and_then(|p| p.file_name())
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_default();
                                }
                            }
                            Err(e) => {
                                d.state = if e.is_cancel() { "cancelled" } else { "failed" };
                                d.error = Some(e.to_string());
                            }
                        }
                        rec.reveal = reveal_items;
                        rec.cancel = None;
                        rec.dto.clone()
                    };
                    self.record_history(&snap);
                    self.emit("transfer-update", snap);
                }
            }
        }
    }

    /// Devices for the right-click submenu: those visible right now, then paired ones that are not.
    pub fn menu_devices(&self) -> Vec<crate::shellmenu::MenuDevice> {
        let mut out: Vec<crate::shellmenu::MenuDevice> = self
            .all_devices()
            .iter()
            .map(|d| (d.id.to_string(), d.name.clone()))
            .collect();
        if let Ok(list) = self.store.list() {
            for t in list {
                let id = t.id.to_string();
                if !out.iter().any(|(i, _)| *i == id) {
                    out.push((id, t.name));
                }
            }
        }
        out.truncate(12);
        out
    }

    /// Keeps the Explorer submenu in step with the devices (only while the setting is on).
    async fn context_menu_loop(&self) {
        let mut last: Vec<crate::shellmenu::MenuDevice> = Vec::new();
        let mut last_heal = Instant::now() - Duration::from_secs(120);
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tick.tick().await;
            let mut compact = crate::shellext::is_registered();
            let wanted = lock(&self.settings).context_menu;
            // You turned the menu on but Windows no longer has it (package removed, Windows update, ...):
            // register it again, at most once every minute.
            if wanted
                && !compact
                && !crate::shellmenu::is_enabled()
                && last_heal.elapsed() > Duration::from_secs(60)
            {
                last_heal = Instant::now();
                if let Ok(exe) = std::env::current_exe() {
                    let devices = self.menu_devices();
                    let ok = tokio::task::spawn_blocking(move || {
                        crate::shellext::register(&exe, &devices)
                    })
                    .await
                    .map(|r| r.is_ok())
                    .unwrap_or(false);
                    compact = ok && crate::shellext::is_registered();
                }
            }
            if !compact && !crate::shellmenu::is_enabled() {
                last.clear();
                continue;
            }
            let now = self.menu_devices();
            if now != last {
                if let Ok(exe) = std::env::current_exe() {
                    let ok = if compact {
                        crate::shellext::write_config(&exe, &now).is_ok()
                    } else {
                        crate::shellmenu::write(&exe, &now).is_ok()
                    };
                    if ok {
                        last = now;
                    }
                }
            }
        }
    }

    /// True while Bolt is only in the tray (window hidden or minimized).
    pub fn window_hidden(&self) -> bool {
        self.app
            .get_webview_window("main")
            .map(|w| !w.is_visible().unwrap_or(true) || w.is_minimized().unwrap_or(false))
            .unwrap_or(false)
    }

    fn attention(&self) {
        if let Some(w) = self.app.get_webview_window("main") {
            // Windows refuses plain focus requests from background apps, so raise the window by
            // briefly making it topmost, then flash the taskbar button as a fallback.
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_always_on_top(true);
            let _ = w.set_focus();
            let _ = w.set_always_on_top(false);
            let _ = w.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
    }

    pub fn history(&self, f: &HistoryFilter) -> Result<Vec<HistoryDto>, String> {
        let q = HistoryQuery {
            text: f.text.clone().filter(|t| !t.trim().is_empty()),
            status: f.status.as_deref().and_then(|s| match s {
                "completed" => Some(Status::Completed),
                "failed" => Some(Status::Failed),
                "cancelled" => Some(Status::Cancelled),
                "rejected" => Some(Status::Rejected),
                _ => None,
            }),
            direction: f.direction.as_deref().and_then(|s| match s {
                "sent" => Some(Direction::Sent),
                "received" => Some(Direction::Received),
                _ => None,
            }),
            limit: Some(500),
        };
        Ok(self
            .store
            .history(&q)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|e| HistoryDto {
                transfer_id: e.transfer_id,
                direction: if e.direction == Direction::Sent {
                    "sent"
                } else {
                    "received"
                },
                peer_id: e.peer_id,
                peer_name: e.peer_name,
                status: status_str(e.status),
                file_count: e.file_count,
                title: e.title,
                total_bytes: e.total_bytes,
                started_unix: e.started_unix,
                finished_unix: e.finished_unix,
                error: e.error,
            })
            .collect())
    }
}

fn load_settings(store: &Store, default_download: &std::path::Path) -> SettingsDto {
    let get = |k: &str| store.get_setting(k).ok().flatten();
    let env_dl = std::env::var("BOLT_DOWNLOAD_DIR").ok();
    SettingsDto {
        device_name: get("device_name")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(default_device_name),
        download_dir: get("download_dir")
            .or(env_dl)
            .unwrap_or_else(|| default_download.to_string_lossy().into_owned()),
        auto_accept_trusted: get("auto_accept_trusted").as_deref() == Some("true"),
        theme: get("theme")
            .filter(|t| matches!(t.as_str(), "dark" | "light" | "system"))
            .unwrap_or_else(|| "dark".into()),
        reduce_motion: get("reduce_motion")
            .filter(|t| matches!(t.as_str(), "system" | "on" | "off"))
            .unwrap_or_else(|| "system".into()),
        notifications: get("notifications").as_deref() != Some("false"),
        start_on_login: false, // filled from the OS by get_settings
        run_in_background: get("run_in_background").as_deref() != Some("false"),
        // what you asked for; get_settings reports the real state (registered or not)
        context_menu: get("context_menu").as_deref() == Some("true"),
    }
}

pub fn save_settings(store: &Store, s: &SettingsDto) -> Result<(), String> {
    let put = |k: &str, v: &str| store.set_setting(k, v).map_err(|e| e.to_string());
    put("device_name", &s.device_name)?;
    put("download_dir", &s.download_dir)?;
    put("auto_accept_trusted", &s.auto_accept_trusted.to_string())?;
    put("theme", &s.theme)?;
    put("reduce_motion", &s.reduce_motion)?;
    put("notifications", &s.notifications.to_string())?;
    put("run_in_background", &s.run_in_background.to_string())?;
    put("context_menu", &s.context_menu.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_living_only_on_this_computer_are_recognised() {
        let mine: Vec<std::net::IpAddr> = vec!["192.168.1.10".parse().unwrap()];
        let set = |v: &[&str]| v.iter().map(|s| s.parse::<SocketAddr>().unwrap()).collect();
        assert!(is_local_only(
            &set(&["192.168.1.10:47600", "127.0.0.1:47600"]),
            &mine
        ));
        assert!(is_local_only(&set(&["[::1]:47600"]), &mine));
        assert!(!is_local_only(&set(&["192.168.1.55:47600"]), &mine));
        assert!(!is_local_only(
            &set(&["192.168.1.10:1", "192.168.1.55:1"]),
            &mine
        ));
        assert!(!is_local_only(&set(&[]), &mine));
    }
}
