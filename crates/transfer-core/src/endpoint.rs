use crate::error::TransferError;
use bolt_protocol::ALPN;
use bolt_security::{tls, DeviceId, Identity};
use quinn::{
    crypto::rustls::{QuicClientConfig, QuicServerConfig},
    Endpoint, IdleTimeout, TransportConfig, VarInt,
};
use rustls::pki_types::CertificateDer;
use std::{net::SocketAddr, sync::Arc, time::Duration};

/// A bound QUIC endpoint that can both accept and initiate connections.
#[derive(Clone)]
pub struct FlashEndpoint {
    pub(crate) endpoint: Endpoint,
    pub(crate) identity: Arc<Identity>,
    /// UDP receive/send buffer sizes the OS actually granted (it may clamp what we asked for).
    buffers: (usize, usize),
}

/// Congestion controller used for the QUIC connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Congestion {
    Cubic,
    Bbr,
    NewReno,
}

/// Network-path tuning. The defaults are what the apps ship with; `BOLT_CC` / `BOLT_MTU` override them
/// for experiments without a rebuild, and [`EndpointTuning::parse`] takes the same settings as a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointTuning {
    pub congestion: Congestion,
    /// Largest UDP payload path-MTU discovery may probe up to. 1452 = the quinn default (no probing past
    /// a normal Ethernet/Wi-Fi MTU); larger values only help on jumbo-frame or loopback paths.
    pub mtu_upper_bound: u16,
}

impl Default for EndpointTuning {
    fn default() -> Self {
        Self {
            // BBR: on lossy Wi-Fi-like paths loss-based Cubic collapses to ~13-16 % of line rate while BBR
            // keeps 63-94 % (see `sim_matrix` in sim.rs); on loopback they are equal.
            congestion: Congestion::Bbr,
            // 1452 = no probing past a normal Ethernet/Wi-Fi MTU. 9000 gave no measurable gain on a 1500-byte
            // path or on loopback, and probing can black-hole on some access points.
            mtu_upper_bound: 1452,
        }
    }
}

impl EndpointTuning {
    /// `"cc=bbr,mtu=1452"` (either key optional, unknown keys ignored).
    pub fn parse(s: &str) -> Self {
        let mut t = Self::default();
        for kv in s.split([',', ';']) {
            let Some((k, v)) = kv.split_once('=') else {
                continue;
            };
            match (k.trim(), v.trim().to_ascii_lowercase().as_str()) {
                ("cc", "bbr") => t.congestion = Congestion::Bbr,
                ("cc", "cubic") => t.congestion = Congestion::Cubic,
                ("cc", "reno") | ("cc", "newreno") => t.congestion = Congestion::NewReno,
                ("mtu", n) => {
                    if let Ok(n) = n.parse::<u32>() {
                        t.mtu_upper_bound = n.clamp(1200, 9000) as u16;
                    }
                }
                _ => {}
            }
        }
        t
    }

    /// Defaults, overridden by the `BOLT_CC` and `BOLT_MTU` environment variables.
    pub fn from_env() -> Self {
        let mut spec = String::new();
        if let Ok(v) = std::env::var("BOLT_CC") {
            spec += &format!("cc={v},");
        }
        if let Ok(v) = std::env::var("BOLT_MTU") {
            spec += &format!("mtu={v}");
        }
        Self::parse(&spec)
    }
}

fn transport(tuning: EndpointTuning) -> Result<Arc<TransportConfig>, TransferError> {
    let mut t = TransportConfig::default();
    match tuning.congestion {
        Congestion::Cubic => {}
        Congestion::Bbr => {
            t.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
        }
        Congestion::NewReno => {
            t.congestion_controller_factory(Arc::new(quinn::congestion::NewRenoConfig::default()));
        }
    }
    t.keep_alive_interval(Some(Duration::from_secs(4)));
    t.max_idle_timeout(Some(
        IdleTimeout::try_from(Duration::from_secs(15))
            .map_err(|e| TransferError::Setup(e.to_string()))?,
    ));
    t.max_concurrent_bidi_streams(4u32.into());
    t.max_concurrent_uni_streams(0u32.into());
    // Large windows keep a LAN link full; memory is still bounded by these caps.
    t.stream_receive_window(VarInt::from_u32(8 * 1024 * 1024));
    t.receive_window(VarInt::from_u32(32 * 1024 * 1024));
    t.send_window(32 * 1024 * 1024);
    // Probe for larger datagrams than the 1452-byte default ceiling (jumbo-frame LANs, loopback).
    let mut mtu = quinn::MtuDiscoveryConfig::default();
    mtu.upper_bound(tuning.mtu_upper_bound);
    t.mtu_discovery_config(Some(mtu));
    Ok(Arc::new(t))
}

/// UDP socket with enlarged buffers. Windows' default receive buffer is small
/// enough that a fast sender causes packet loss, which collapses QUIC's
/// congestion window even on loopback.
fn udp_socket(addr: SocketAddr) -> std::io::Result<(std::net::UdpSocket, usize, usize)> {
    use socket2::{Domain, Protocol, Socket, Type};
    const BUF: usize = 8 * 1024 * 1024;
    let socket = Socket::new(Domain::for_address(addr), Type::DGRAM, Some(Protocol::UDP))?;
    // Best effort: the OS may clamp these.
    let _ = socket.set_recv_buffer_size(BUF);
    let _ = socket.set_send_buffer_size(BUF);
    socket.bind(&addr.into())?;
    let granted = (
        socket.recv_buffer_size().unwrap_or(0),
        socket.send_buffer_size().unwrap_or(0),
    );
    Ok((socket.into(), granted.0, granted.1))
}

impl FlashEndpoint {
    pub fn bind(identity: Arc<Identity>, addr: SocketAddr) -> Result<Self, TransferError> {
        Self::bind_with(identity, addr, EndpointTuning::from_env())
    }

    pub fn bind_with(
        identity: Arc<Identity>,
        addr: SocketAddr,
        tuning: EndpointTuning,
    ) -> Result<Self, TransferError> {
        let setup = |e: &dyn std::fmt::Display| TransferError::Setup(e.to_string());
        let server_tls = tls::server_config(&identity, ALPN).map_err(|e| setup(&e))?;
        let client_tls = tls::client_config(&identity, ALPN).map_err(|e| setup(&e))?;
        let transport = transport(tuning)?;

        let mut server = quinn::ServerConfig::with_crypto(Arc::new(
            QuicServerConfig::try_from(server_tls).map_err(|e| setup(&e))?,
        ));
        server.transport_config(transport.clone());

        let mut client = quinn::ClientConfig::new(Arc::new(
            QuicClientConfig::try_from(client_tls).map_err(|e| setup(&e))?,
        ));
        client.transport_config(transport);

        let (socket, rcv, snd) = udp_socket(addr)?;
        tracing::info!(target: "bolt::timing", rcv_buffer = rcv, snd_buffer = snd, ?tuning, "UDP socket buffers granted by the OS");
        let mut endpoint = Endpoint::new(
            quinn::EndpointConfig::default(),
            Some(server),
            socket,
            quinn::default_runtime()
                .ok_or_else(|| TransferError::Setup("no async runtime".into()))?,
        )?;
        endpoint.set_default_client_config(client);
        Ok(Self {
            endpoint,
            identity,
            buffers: (rcv, snd),
        })
    }

    /// `(receive, send)` UDP buffer bytes the OS granted. Far below the 8 MiB requested (typical clamp on
    /// phones: ~200 KiB–2 MiB) means bursts overflow the socket and show up as packet loss.
    pub fn udp_buffers(&self) -> (usize, usize) {
        self.buffers
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.endpoint.local_addr()
    }

    pub fn device_id(&self) -> &DeviceId {
        self.identity.device_id()
    }

    pub fn close(&self) {
        self.endpoint
            .close(VarInt::from_u32(crate::CLOSE_DONE), b"shutdown");
    }
}

/// Identity of the remote end of an established connection (from its TLS certificate).
pub fn peer_id(conn: &quinn::Connection) -> Option<DeviceId> {
    let any = conn.peer_identity()?;
    let certs = any.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    certs.first().map(DeviceId::from_cert)
}

/// Is a Bolt receiver reachable at `addr`? Completes the TLS handshake (and checks the peer's
/// identity when `expected` is given), then closes — no data is exchanged. Used to hide stale
/// mDNS entries for devices that went away without saying goodbye.
pub async fn probe(
    ep: &FlashEndpoint,
    addr: SocketAddr,
    expected: Option<&DeviceId>,
    wait: Duration,
) -> bool {
    let Ok(connecting) = ep.endpoint.connect(addr, "bolt.local") else {
        return false;
    };
    match tokio::time::timeout(wait, connecting).await {
        Ok(Ok(conn)) => {
            let ok = expected.is_none_or(|e| peer_id(&conn).as_ref() == Some(e));
            conn.close(VarInt::from_u32(crate::CLOSE_DONE), b"probe");
            ok
        }
        _ => false,
    }
}
