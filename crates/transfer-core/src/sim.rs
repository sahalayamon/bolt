//! A small in-process "bad network": a UDP relay with a rate limit, one-way delay, random loss, a
//! tail-drop queue and an MTU cap, standing between a real QUIC client and server. It lets the
//! congestion-controller and MTU choices be compared on a *controlled, repeatable* path.
//!
//! This is a model, not a Wi-Fi measurement: real radios add retries, aggregation and airtime
//! contention that this does not reproduce. It answers "which setting copes better with loss and a
//! small MTU", not "how fast is your phone".

use crate::*;
use bolt_protocol::DeviceKind;
use bolt_security::{Identity, MemoryTrustStore};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy)]
pub struct Link {
    pub mbps: f64,
    pub one_way_delay: Duration,
    pub loss: f64,
    /// Datagrams larger than this are dropped (a path MTU smaller than what was probed).
    pub mtu: usize,
    /// Tail-drop once the queue would add more than this much delay.
    pub queue: Duration,
}

struct Shaper {
    link: Link,
    free_at: Mutex<Instant>,
}

impl Shaper {
    fn new(link: Link) -> Arc<Self> {
        Arc::new(Self {
            link,
            free_at: Mutex::new(Instant::now()),
        })
    }

    /// When should this packet leave, or `None` if the path drops it.
    fn admit(&self, len: usize) -> Option<Instant> {
        if len > self.link.mtu || rand::random::<f64>() < self.link.loss {
            return None;
        }
        let now = Instant::now();
        let tx = Duration::from_secs_f64(len as f64 * 8.0 / (self.link.mbps * 1e6));
        let mut free = self.free_at.lock().unwrap();
        let start = (*free).max(now);
        if start - now > self.link.queue {
            return None;
        }
        *free = start + tx;
        Some(start + tx + self.link.one_way_delay)
    }
}

/// One direction of the path: packets are admitted by the shaper and then released strictly in order
/// (a per-packet timer task would let neighbours overtake each other, which QUIC reads as loss).
fn spawn_pipe(
    shaper: Arc<Shaper>,
    mut rx_pkts: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    out: Arc<UdpSocket>,
    to: Arc<Mutex<Option<SocketAddr>>>,
) {
    let (tx_q, mut rx_q) = tokio::sync::mpsc::unbounded_channel::<(Instant, Vec<u8>)>();
    tokio::spawn(async move {
        while let Some(pkt) = rx_pkts.recv().await {
            if let Some(at) = shaper.admit(pkt.len()) {
                let _ = tx_q.send((at, pkt));
            }
        }
    });
    tokio::spawn(async move {
        while let Some((at, pkt)) = rx_q.recv().await {
            if at > Instant::now() {
                tokio::time::sleep_until(at.into()).await;
            }
            let dest = *to.lock().unwrap();
            if let Some(addr) = dest {
                let _ = out.send_to(&pkt, addr).await;
            }
        }
    });
}

/// Start a relay in front of `server`; returns the address the client should dial.
pub async fn relay(link: Link, server: SocketAddr) -> SocketAddr {
    let front = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let back = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let front_addr = front.local_addr().unwrap();
    let client: Arc<Mutex<Option<SocketAddr>>> = Arc::default();
    let server_addr: Arc<Mutex<Option<SocketAddr>>> = Arc::new(Mutex::new(Some(server)));

    let (up_tx, up_rx) = tokio::sync::mpsc::unbounded_channel();
    let (down_tx, down_rx) = tokio::sync::mpsc::unbounded_channel();
    spawn_pipe(Shaper::new(link), up_rx, back.clone(), server_addr);
    spawn_pipe(Shaper::new(link), down_rx, front.clone(), client.clone());

    {
        let front = front.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            while let Ok((n, from)) = front.recv_from(&mut buf).await {
                *client.lock().unwrap() = Some(from);
                let _ = up_tx.send(buf[..n].to_vec());
            }
        });
    }
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        while let Ok((n, _from)) = back.recv_from(&mut buf).await {
            let _ = down_tx.send(buf[..n].to_vec());
        }
    });
    front_addr
}

fn local(name: &str) -> LocalInfo {
    LocalInfo {
        name: name.into(),
        kind: DeviceKind::Desktop,
    }
}

/// Run the link test through `link` with both ends using `tuning`.
pub async fn measure(
    link: Link,
    tuning: EndpointTuning,
    bytes: u64,
) -> Result<LinkReport, TransferError> {
    let server_ep = FlashEndpoint::bind_with(
        Arc::new(Identity::generate().unwrap()),
        "127.0.0.1:0".parse().unwrap(),
        tuning,
    )
    .unwrap();
    let client_ep = FlashEndpoint::bind_with(
        Arc::new(Identity::generate().unwrap()),
        "127.0.0.1:0".parse().unwrap(),
        tuning,
    )
    .unwrap();
    let server_addr = server_ep.local_addr().unwrap();
    let server_id = server_ep.device_id().clone();
    let (_handle, _events) = Receiver::start(
        server_ep,
        ReceiverConfig {
            download_dir: std::env::temp_dir().join("bolt-sim-unused"),
            local: local("sim-server"),
            auto_accept_trusted: false,
            max_concurrent_transfers: 2,
            transfer: TransferConfig::default(),
        },
        Arc::new(MemoryTrustStore::default()),
    );
    let via = relay(link, server_addr).await;
    link_test(
        &client_ep,
        via,
        Some(&server_id),
        &local("sim-client"),
        bytes,
        &TransferConfig {
            connect_timeout: Duration::from_secs(15),
            ..Default::default()
        },
        CancellationToken::new(),
    )
    .await
}

#[tokio::test]
async fn link_test_runs_through_an_impaired_path() {
    let link = Link {
        mbps: 200.0,
        one_way_delay: Duration::from_millis(2),
        loss: 0.005,
        mtu: 1500,
        queue: Duration::from_millis(60),
    };
    let r = measure(link, EndpointTuning::default(), 4 * 1024 * 1024)
        .await
        .unwrap();
    assert!(r.up.bytes == 4 * 1024 * 1024 && r.down.bytes == 4 * 1024 * 1024);
    // 200 Mbit/s = 25 MB/s ceiling; just require it to be a plausible number below the ceiling.
    assert!(
        r.up.mb_per_s() > 0.2 && r.up.mb_per_s() < 26.0,
        "{:?}",
        r.up
    );
    assert!(
        r.down.mb_per_s() > 0.2 && r.down.mb_per_s() < 26.0,
        "{:?}",
        r.down
    );
}

#[test]
fn tuning_parses_and_clamps() {
    let t = EndpointTuning::parse("cc=bbr, mtu=1452");
    assert_eq!(t.congestion, Congestion::Bbr);
    assert_eq!(t.mtu_upper_bound, 1452);
    assert_eq!(EndpointTuning::parse("mtu=99999").mtu_upper_bound, 9000);
    assert_eq!(EndpointTuning::parse("mtu=10").mtu_upper_bound, 1200);
    assert_eq!(EndpointTuning::parse("junk"), EndpointTuning::default());
    assert_eq!(EndpointTuning::default().congestion, Congestion::Bbr);
    assert_eq!(
        EndpointTuning::parse("cc=cubic").congestion,
        Congestion::Cubic
    );
}

/// `cargo test -p bolt-transfer-core --release -- --ignored --nocapture sim_matrix`
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "benchmark: takes a few minutes"]
async fn sim_matrix() {
    let scenarios = [
        (
            "clean LAN   100 Mbit/s,  2 ms RTT, 0% loss, MTU 1500",
            Link {
                mbps: 100.0,
                one_way_delay: Duration::from_millis(1),
                loss: 0.0,
                mtu: 1500,
                queue: Duration::from_millis(40),
            },
        ),
        (
            "good Wi-Fi   60 Mbit/s,  6 ms RTT, 0.5% loss, MTU 1500",
            Link {
                mbps: 60.0,
                one_way_delay: Duration::from_millis(3),
                loss: 0.005,
                mtu: 1500,
                queue: Duration::from_millis(60),
            },
        ),
        (
            "lossy Wi-Fi  30 Mbit/s, 10 ms RTT, 2% loss, MTU 1500",
            Link {
                mbps: 30.0,
                one_way_delay: Duration::from_millis(5),
                loss: 0.02,
                mtu: 1500,
                queue: Duration::from_millis(80),
            },
        ),
        (
            "poor Wi-Fi   15 Mbit/s, 20 ms RTT, 4% loss, MTU 1500",
            Link {
                mbps: 15.0,
                one_way_delay: Duration::from_millis(10),
                loss: 0.04,
                mtu: 1500,
                queue: Duration::from_millis(100),
            },
        ),
    ];
    let tunings = [
        ("cubic mtu9000", "cc=cubic,mtu=9000"),
        ("cubic mtu1452", "cc=cubic,mtu=1452"),
        ("bbr   mtu9000", "cc=bbr,mtu=9000"),
        ("bbr   mtu1452", "cc=bbr,mtu=1452"),
    ];
    println!();
    for (name, link) in scenarios {
        println!("{name}");
        let bytes: u64 = if link.mbps >= 60.0 {
            12_000_000
        } else if link.mbps >= 30.0 {
            6_000_000
        } else {
            4_000_000
        };
        for (label, spec) in tunings {
            let run = tokio::time::timeout(
                Duration::from_secs(120),
                measure(link, EndpointTuning::parse(spec), bytes),
            )
            .await;
            match run {
                Ok(Ok(r)) => println!(
                    "  {label}: up {:6.2} MB/s ({:5.1}% of line) loss {:4.1}% mtu {} | down {:6.2} MB/s loss {:4.1}% mtu {}",
                    r.up.mb_per_s(),
                    r.up.mbit_per_s() * 100.0 / link.mbps,
                    r.up.path.loss_pct(),
                    r.up.path.mtu,
                    r.down.mb_per_s(),
                    r.down.path.loss_pct(),
                    r.down.path.mtu,
                ),
                Ok(Err(e)) => println!("  {label}: FAILED ({e})"),
                Err(_) => println!("  {label}: FAILED (no result in 120 s)"),
            }
        }
    }
}

/// Diagnose a single scenario: `cargo test -p bolt-transfer-core --release --lib -- --ignored --nocapture sim_one`
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "diagnostic"]
async fn sim_one() {
    let link = Link {
        mbps: 60.0,
        one_way_delay: Duration::from_millis(3),
        loss: 0.005,
        mtu: 1500,
        queue: Duration::from_millis(60),
    };
    for spec in ["cc=cubic,mtu=1452", "cc=bbr,mtu=1452"] {
        let t = Instant::now();
        let r = measure(link, EndpointTuning::parse(spec), 12_000_000).await;
        match r {
            Ok(r) => println!(
                "{spec}: ok in {:.1}s up {:.2} MB/s down {:.2} MB/s | up path: {}",
                t.elapsed().as_secs_f64(),
                r.up.mb_per_s(),
                r.down.mb_per_s(),
                r.up.path
            ),
            Err(e) => println!(
                "{spec}: FAILED after {:.1}s: {e}",
                t.elapsed().as_secs_f64()
            ),
        }
    }
}
