//! `bolt` — headless client used to prove the real network path between
//! two machines/processes, and to benchmark it. The desktop and Android apps
//! use the same crates.
//!
//! Usage:
//!   bolt [--data DIR] id
//!   bolt [--data DIR] receive [--dir DIR] [--port N] [--name NAME] [--pair] [--auto-accept-trusted]
//!   bolt [--data DIR] send --to IP[:PORT] [--peer DEVICE_ID] [--name NAME] [--text TEXT_OR_LINK] [PATH...]
//!   bolt [--data DIR] pair --uri 'bolt://pair?...' [--name NAME]
//!   bolt discover [--secs N]
//!   bolt bench [--mib N]

use bolt_discovery::{parse_manual_endpoint, Advertisement, Discovery, DEFAULT_PORT};
use bolt_protocol::DeviceKind;
use bolt_security::{DeviceId, Identity, PairingUri, TrustStore};
use bolt_transfer_core::*;
use bolt_transfer_store::{Direction, HistoryEntry, Status, Store};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

struct Args(Vec<String>);

impl Args {
    fn flag(&mut self, name: &str) -> bool {
        self.0
            .iter()
            .position(|a| a == name)
            .map(|i| self.0.remove(i))
            .is_some()
    }
    fn opt(&mut self, name: &str) -> Option<String> {
        let i = self.0.iter().position(|a| a == name)?;
        self.0.remove(i);
        (i < self.0.len()).then(|| self.0.remove(i))
    }
}

fn default_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Bolt CLI".into())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn mb_s(bytes: u64, d: Duration) -> f64 {
    bytes as f64 / 1_000_000.0 / d.as_secs_f64().max(1e-9)
}

#[tokio::main]
async fn main() -> Res<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let mut args = Args(std::env::args().skip(1).collect());
    let data: PathBuf = args
        .opt("--data")
        .map(Into::into)
        .unwrap_or_else(|| PathBuf::from(".bolt-data"));
    let Some(cmd) = (!args.0.is_empty()).then(|| args.0.remove(0)) else {
        eprintln!("{}", include_str!("usage.txt"));
        return Ok(());
    };
    match cmd.as_str() {
        "id" => {
            let ident = Identity::load_or_create(&data)?;
            println!("{}", ident.device_id());
        }
        "receive" => receive(&data, args).await?,
        "send" => send(&data, args).await?,
        "pair" => pair(&data, args).await?,
        "discover" => discover(args).await?,
        "bench" => bench(args).await?,
        "linktest" => linktest(&data, args).await?,
        _ => eprintln!("{}", include_str!("usage.txt")),
    }
    Ok(())
}

async fn receive(data: &std::path::Path, mut args: Args) -> Res<()> {
    let dir: PathBuf = args
        .opt("--dir")
        .map(Into::into)
        .unwrap_or_else(|| PathBuf::from("bolt-downloads"));
    let port: u16 = args
        .opt("--port")
        .map(|p| p.parse())
        .transpose()?
        .unwrap_or(DEFAULT_PORT);
    let name = args.opt("--name").unwrap_or_else(default_name);
    let want_pair = args.flag("--pair");
    let auto = args.flag("--auto-accept-trusted");

    let ident = Arc::new(Identity::load_or_create(data)?);
    let store = Arc::new(Store::open(&data.join("bolt.sqlite"))?);
    let ep = FlashEndpoint::bind(ident.clone(), format!("0.0.0.0:{port}").parse()?)
        .or_else(|_| FlashEndpoint::bind(ident.clone(), "0.0.0.0:0".parse().unwrap()))?;
    let bound = ep.local_addr()?;
    let cfg = ReceiverConfig {
        download_dir: dir.clone(),
        local: LocalInfo {
            name: name.clone(),
            kind: DeviceKind::Desktop,
        },
        auto_accept_trusted: auto,
        max_concurrent_transfers: 3,
        transfer: TransferConfig::default(),
    };
    let (handle, mut events) = Receiver::start(ep, cfg, store.clone());

    let _disc = match Discovery::start(&Advertisement {
        id: ident.device_id().clone(),
        name: name.clone(),
        kind: DeviceKind::Desktop,
        port: bound.port(),
    }) {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("mDNS unavailable ({e}); use manual connection by IP:port");
            None
        }
    };
    println!(
        "Bolt receiving as '{name}' on port {} -> {}",
        bound.port(),
        dir.display()
    );
    println!("device id: {}", ident.device_id());
    if want_pair {
        let ip = std::env::var("BOLT_ADVERTISE_IP").unwrap_or_else(|_| "127.0.0.1".into());
        let uri = handle.begin_pairing(Duration::from_secs(300), format!("{ip}:{}", bound.port()));
        println!("pairing (valid 5 min): {}", uri.encode());
    }

    while let Some(ev) = events.recv().await {
        match ev {
            IncomingEvent::OfferReceived(o) => {
                println!(
                    "\nIncoming from '{}' ({}): {} file(s), {} bytes{}",
                    o.peer.name,
                    o.peer.id.short(),
                    o.offer.files.len(),
                    o.total_bytes,
                    if o.trusted {
                        " [trusted]"
                    } else {
                        " [UNKNOWN DEVICE]"
                    }
                );
                for f in o.offer.files.iter().take(10) {
                    println!("  - {} ({} bytes)", f.path, f.size);
                }
                if prompt("Accept? [y/N] ").await {
                    o.accept()
                } else {
                    o.reject("declined by user")
                }
            }
            IncomingEvent::PairRequested(p) => {
                println!(
                    "\nPairing request from '{}'. Verification code: {}",
                    p.peer.name, p.code
                );
                if prompt("Does the other device show the same code? Approve? [y/N] ").await {
                    p.approve()
                } else {
                    p.deny()
                }
            }
            IncomingEvent::TransferStarted { progress, .. } => {
                tokio::spawn(report_progress(progress, "receiving"));
            }
            IncomingEvent::TransferFinished {
                transfer_id,
                peer,
                result,
            } => {
                let (status, err, files, bytes, title) = match &result {
                    Ok(s) => (
                        Status::Completed,
                        None,
                        s.files,
                        s.bytes,
                        s.saved_paths
                            .first()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ),
                    Err(e) => (
                        if e.is_cancel() {
                            Status::Cancelled
                        } else {
                            Status::Failed
                        },
                        Some(e.to_string()),
                        0,
                        0,
                        String::new(),
                    ),
                };
                if let Ok(s) = &result {
                    for t in &s.texts {
                        println!("\ntext received:\n{t}");
                    }
                }
                match &result {
                    Ok(s) => println!(
                        "\nreceived {} file(s), {} bytes in {:.2}s ({:.1} MB/s), verified\n  stages: {}\n  path:   {}",
                        s.files,
                        s.bytes,
                        s.elapsed.as_secs_f64(),
                        mb_s(s.bytes, s.elapsed),
                        s.timings,
                        s.path
                    ),
                    Err(e) => println!("\ntransfer failed: {e}"),
                }
                store.record_transfer(&HistoryEntry {
                    transfer_id,
                    direction: Direction::Received,
                    peer_id: peer.id.to_string(),
                    peer_name: peer.name,
                    status,
                    file_count: files,
                    title,
                    total_bytes: bytes,
                    started_unix: now(),
                    finished_unix: now(),
                    error: err,
                })?;
            }
        }
    }
    Ok(())
}

async fn prompt(q: &str) -> bool {
    let q = q.to_owned();
    tokio::task::spawn_blocking(move || {
        use std::io::{BufRead, Write};
        print!("{q}");
        let _ = std::io::stdout().flush();
        let mut s = String::new();
        std::io::stdin().lock().read_line(&mut s).is_ok() && s.trim().eq_ignore_ascii_case("y")
    })
    .await
    .unwrap_or(false)
}

async fn report_progress(mut rx: tokio::sync::watch::Receiver<Progress>, verb: &'static str) {
    let mut meter = SpeedMeter::default();
    while rx.changed().await.is_ok() {
        let p = rx.borrow().clone();
        let speed = meter.sample(Instant::now(), p.bytes_done);
        let eta = meter
            .eta(p.bytes_total.saturating_sub(p.bytes_done))
            .map(|d| format!("{}s", d.as_secs()))
            .unwrap_or_else(|| "-".into());
        print!(
            "\r{verb}: {:>5.1}%  {:>7.1} MB/s  eta {eta}  file {}/{}   ",
            p.bytes_done as f64 * 100.0 / p.bytes_total.max(1) as f64,
            speed / 1e6,
            p.files_done,
            p.files_total
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();
        if matches!(
            p.state,
            TransferState::Completed
                | TransferState::Failed
                | TransferState::Cancelled
                | TransferState::Rejected
        ) {
            println!();
            break;
        }
    }
}

async fn client_endpoint(data: &std::path::Path) -> Res<(FlashEndpoint, Arc<Store>)> {
    let ident = Arc::new(Identity::load_or_create(data)?);
    let store = Arc::new(Store::open(&data.join("bolt.sqlite"))?);
    Ok((FlashEndpoint::bind(ident, "0.0.0.0:0".parse()?)?, store))
}

async fn send(data: &std::path::Path, mut args: Args) -> Res<()> {
    let to = parse_manual_endpoint(&args.opt("--to").ok_or("--to IP[:PORT] is required")?)?;
    let peer = args
        .opt("--peer")
        .map(|p| DeviceId::parse(&p))
        .transpose()?;
    let name = args.opt("--name").unwrap_or_else(default_name);
    let text = args.opt("--text");
    let paths: Vec<PathBuf> = args.0.iter().map(PathBuf::from).collect();
    if paths.is_empty() && text.is_none() {
        return Err("give at least one file or folder, or --text \"...\"".into());
    }
    let mut items = scan(&paths)?;
    if let Some(t) = &text {
        items.push(bolt_transfer_core::SendItem::text(t, 0)?);
    }
    let (ep, store) = client_endpoint(data).await?;
    let (reporter, rx) = ProgressReporter::new();
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        c2.cancel();
    });
    tokio::spawn(report_progress(rx, "sending"));
    let title = items
        .first()
        .map(|i| i.rel_path.clone())
        .unwrap_or_default();
    let total: u64 = items.iter().map(|i| i.size).sum();
    let count = items.len() as u32;
    let started = now();
    let req = SendRequest {
        addr: to,
        expected_peer: peer,
        local: LocalInfo {
            name,
            kind: DeviceKind::Desktop,
        },
        items,
        config: TransferConfig::default(),
    };
    let result = send_files(&ep, req, cancel, reporter).await;
    let (status, err, id, peer_info) = match &result {
        Ok(s) => (
            Status::Completed,
            None,
            s.transfer_id.clone(),
            Some(s.peer.clone()),
        ),
        Err(TransferError::Rejected(r)) => (Status::Rejected, Some(r.clone()), String::new(), None),
        Err(e) => (
            if e.is_cancel() {
                Status::Cancelled
            } else {
                Status::Failed
            },
            Some(e.to_string()),
            String::new(),
            None,
        ),
    };
    if let Some(p) = peer_info {
        store.record_transfer(&HistoryEntry {
            transfer_id: id,
            direction: Direction::Sent,
            peer_id: p.id.to_string(),
            peer_name: p.name,
            status,
            file_count: count,
            title,
            total_bytes: total,
            started_unix: started,
            finished_unix: now(),
            error: err,
        })?;
    }
    match result {
        Ok(s) => println!("sent {} file(s), {} bytes to '{}' in {:.2}s ({:.1} MB/s) — receiver verified every file\n  stages: {}\n  path:   {}", s.files, s.bytes, s.peer.name, s.elapsed.as_secs_f64(), mb_s(s.bytes, s.elapsed), s.timings, s.path),
        Err(e) => {
            eprintln!("send failed: {e}");
            std::process::exit(1);
        }
    }
    Ok(())
}

async fn pair(data: &std::path::Path, mut args: Args) -> Res<()> {
    let uri = PairingUri::parse(&args.opt("--uri").ok_or("--uri is required")?)
        .map_err(|e| e.to_string())?;
    let name = args.opt("--name").unwrap_or_else(default_name);
    let addr = parse_manual_endpoint(&uri.addr)?;
    let (ep, store) = client_endpoint(data).await?;
    let local = LocalInfo {
        name,
        kind: DeviceKind::Desktop,
    };
    let peer = pair_with(&ep, addr, &uri, &local, &TransferConfig::default(), store.as_ref(), |code| {
        println!("Verification code: {code}\nConfirm the same code appears on the other device and approve it there.");
    })
    .await?;
    println!(
        "paired with '{}' ({}). Trusted devices: {}",
        peer.name,
        peer.id.short(),
        store.list()?.len()
    );
    Ok(())
}

/// Pure network test: no disk, no hashing. Compare with a real transfer to see where the time goes.
async fn linktest(data: &std::path::Path, mut args: Args) -> Res<()> {
    let to = parse_manual_endpoint(&args.opt("--to").ok_or("--to IP[:PORT] is required")?)?;
    let peer = args
        .opt("--peer")
        .map(|p| DeviceId::parse(&p))
        .transpose()?;
    let mb: u64 = args
        .opt("--mb")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(64);
    let (ep, _store) = client_endpoint(data).await?;
    let local = LocalInfo {
        name: default_name(),
        kind: DeviceKind::Desktop,
    };
    println!("link test to {to}: {mb} MB each way (cc/mtu from BOLT_CC / BOLT_MTU)…");
    let r = link_test(
        &ep,
        to,
        peer.as_ref(),
        &local,
        mb * 1_000_000,
        &TransferConfig::default(),
        CancellationToken::new(),
    )
    .await?;
    println!(
        "connect + handshake: {:.0} ms",
        r.connect.as_secs_f64() * 1000.0
    );
    println!(
        "  up   (this -> peer): {:7.2} MB/s = {:6.1} Mbit/s   [{}]",
        r.up.mb_per_s(),
        r.up.mbit_per_s(),
        r.up.path
    );
    println!(
        "  down (peer -> this): {:7.2} MB/s = {:6.1} Mbit/s   [{}]",
        r.down.mb_per_s(),
        r.down.mbit_per_s(),
        r.down.path
    );
    Ok(())
}

async fn discover(mut args: Args) -> Res<()> {
    let secs: u64 = args
        .opt("--secs")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(5);
    let me = Identity::generate()?;
    let d = Discovery::start(&Advertisement {
        id: me.device_id().clone(),
        name: "discover-probe".into(),
        kind: DeviceKind::Unknown,
        port: 1,
    })?;
    tokio::time::sleep(Duration::from_secs(secs)).await;
    let devs = d.devices().borrow().clone();
    if devs.is_empty() {
        println!("no Bolt devices found (multicast may be blocked on this network; use manual IP)");
    }
    for dev in devs {
        println!(
            "{} [{:?}] {} -> {:?}",
            dev.name,
            dev.kind,
            dev.id.short(),
            dev.addrs
        );
    }
    d.stop();
    Ok(())
}

/// Loopback throughput over the real QUIC+TLS+BLAKE3+disk path. Numbers are
/// measured, include fsync and verification, and depend on this machine's CPU and disk.
async fn bench(mut args: Args) -> Res<()> {
    let mib: usize = args
        .opt("--mib")
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(1024);
    let work = std::env::temp_dir().join(format!("bolt-bench-{}", std::process::id()));
    std::fs::create_dir_all(&work)?;
    let src = work.join("bench.bin");
    {
        use std::io::Write;
        let mut f = std::io::BufWriter::new(std::fs::File::create(&src)?);
        let block: Vec<u8> = (0..1024 * 1024u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8)
            .collect();
        for _ in 0..mib {
            f.write_all(&block)?;
        }
    }
    let rx_ep = FlashEndpoint::bind(Arc::new(Identity::generate()?), "127.0.0.1:0".parse()?)?;
    let tx_ep = FlashEndpoint::bind(Arc::new(Identity::generate()?), "127.0.0.1:0".parse()?)?;
    let addr = rx_ep.local_addr()?;
    let rx_id = rx_ep.device_id().clone();
    let cfg = ReceiverConfig {
        download_dir: work.join("out"),
        local: LocalInfo {
            name: "bench-rx".into(),
            kind: DeviceKind::Desktop,
        },
        auto_accept_trusted: false,
        max_concurrent_transfers: 1,
        transfer: TransferConfig::default(),
    };
    let (_h, mut ev) = Receiver::start(
        rx_ep,
        cfg,
        Arc::new(bolt_security::MemoryTrustStore::default()),
    );
    tokio::spawn(async move {
        while let Some(e) = ev.recv().await {
            if let IncomingEvent::OfferReceived(o) = e {
                o.accept();
            }
        }
    });
    let items = scan(std::slice::from_ref(&src))?;
    let (rep, _rx) = ProgressReporter::new();
    let req = SendRequest {
        addr,
        expected_peer: Some(rx_id),
        local: LocalInfo {
            name: "bench-tx".into(),
            kind: DeviceKind::Desktop,
        },
        items,
        config: TransferConfig::default(),
    };
    let s = send_files(&tx_ep, req, CancellationToken::new(), rep).await?;
    println!("loopback, {} MiB, 1 file: {:.2}s => {:.0} MB/s (sender-side wall clock, receiver fsynced + BLAKE3-verified)", mib, s.elapsed.as_secs_f64(), mb_s(s.bytes, s.elapsed));
    let _ = std::fs::remove_dir_all(&work);
    Ok(())
}
