//! End-to-end tests over real QUIC on loopback. Peers here are real
//! `FlashEndpoint`s in the same process — no mocked transport.

use crate::*;
use bolt_protocol::{
    read_message, write_message, DeviceKind, FileComplete, FileOffer, FileStart, Message,
    TransferOffer, PROTOCOL_VERSION,
};
use bolt_security::{DeviceId, Identity, MemoryTrustStore, TrustStore, TrustedDevice};
use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{sync::mpsc, time::timeout};
use tokio_util::sync::CancellationToken;

const T: Duration = Duration::from_secs(30);

struct Node {
    ep: FlashEndpoint,
    trust: Arc<MemoryTrustStore>,
    dir: tempfile::TempDir,
}

impl Node {
    fn new() -> Self {
        let ident = Arc::new(Identity::generate().unwrap());
        let ep = FlashEndpoint::bind(ident, "127.0.0.1:0".parse().unwrap()).unwrap();
        Self {
            ep,
            trust: Arc::new(MemoryTrustStore::default()),
            dir: tempfile::tempdir().unwrap(),
        }
    }
    fn id(&self) -> DeviceId {
        self.ep.device_id().clone()
    }
    fn addr(&self) -> SocketAddr {
        self.ep.local_addr().unwrap()
    }
    fn downloads(&self) -> PathBuf {
        self.dir.path().join("downloads")
    }
    fn local(&self, name: &str) -> LocalInfo {
        LocalInfo {
            name: name.into(),
            kind: DeviceKind::Desktop,
        }
    }
    fn start_receiver(&self, auto: bool) -> (ReceiverHandle, mpsc::Receiver<IncomingEvent>) {
        let cfg = ReceiverConfig {
            download_dir: self.downloads(),
            local: self.local("receiver"),
            auto_accept_trusted: auto,
            max_concurrent_transfers: 4,
            transfer: TransferConfig {
                approval_timeout: Duration::from_secs(5),
                ..Default::default()
            },
        };
        Receiver::start(self.ep.clone(), cfg, self.trust.clone())
    }
    fn trust_peer(&self, id: &DeviceId) {
        self.trust
            .trust(TrustedDevice {
                id: id.clone(),
                name: "p".into(),
                paired_at_unix: 0,
                last_seen_unix: None,
            })
            .unwrap();
    }
}

fn request(to: &Node, from: &Node, items: Vec<SendItem>) -> SendRequest {
    SendRequest {
        addr: to.addr(),
        expected_peer: Some(to.id()),
        local: from.local("sender"),
        items,
        config: TransferConfig {
            approval_timeout: Duration::from_secs(5),
            ..Default::default()
        },
    }
}

/// Answer every offer with `accept`; forward finished transfers to the returned channel.
fn auto_user(
    mut events: mpsc::Receiver<IncomingEvent>,
    accept: bool,
) -> mpsc::UnboundedReceiver<Result<ReceivedSummary, String>> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(ev) = events.recv().await {
            match ev {
                IncomingEvent::OfferReceived(o) => {
                    if accept {
                        o.accept()
                    } else {
                        o.reject("no thanks")
                    }
                }
                IncomingEvent::TransferFinished { result, .. } => {
                    let _ = tx.send(result.map_err(|e| e.to_string()));
                }
                IncomingEvent::PairRequested(p) => p.approve(),
                IncomingEvent::TransferStarted { .. } => {}
            }
        }
    });
    rx
}

fn write_file(dir: &Path, rel: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, bytes).unwrap();
    p
}

fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| {
            (i as u8)
                .wrapping_mul(31)
                .wrapping_add(seed)
                .wrapping_add((i >> 8) as u8)
        })
        .collect()
}

async fn send(
    to: &Node,
    from: &Node,
    items: Vec<SendItem>,
) -> Result<TransferSummary, TransferError> {
    let (rep, _rx) = ProgressReporter::new();
    timeout(
        T,
        send_files(
            &from.ep,
            request(to, from, items),
            CancellationToken::new(),
            rep,
        ),
    )
    .await
    .expect("send timed out")
}

#[tokio::test]
async fn unknown_sender_is_prompted_and_files_arrive_intact() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (_h, events) = rx_node.start_receiver(false);
    let mut finished = auto_user(events, true);

    let src = tx_node.dir.path().join("src");
    let big = pattern(3 * 1024 * 1024 + 17, 7);
    write_file(&src, "album/big.bin", &big);
    write_file(&src, "album/sub/empty.txt", b"");
    write_file(&src, "album/日本語 ファイル.txt", "héllo".as_bytes());
    let items = scan(&[src.join("album")]).unwrap();

    let summary = send(&rx_node, &tx_node, items).await.unwrap();
    assert_eq!(summary.files, 3);
    assert_eq!(summary.peer.id, rx_node.id());
    // stage accounting: every payload byte went through the timed network write, and time was attributed
    assert_eq!(summary.timings.bytes, summary.bytes);
    assert!(summary.timings.net_calls > 0 && summary.timings.total() > Duration::ZERO);
    assert!(summary.path.mtu >= 1200);

    let done = timeout(T, finished.recv()).await.unwrap().unwrap().unwrap();
    assert_eq!(done.saved_paths.len(), 3);
    assert_eq!(done.timings.bytes, done.bytes);
    assert!(done.timings.disk > Duration::ZERO || done.timings.sync > Duration::ZERO);
    let d = rx_node.downloads().join("album");
    assert_eq!(fs::read(d.join("big.bin")).unwrap(), big);
    assert_eq!(fs::read(d.join("sub/empty.txt")).unwrap(), b"");
    assert_eq!(
        fs::read_to_string(d.join("日本語 ファイル.txt")).unwrap(),
        "héllo"
    );
    // no staging residue after a completed batch
    let leftovers: Vec<_> = fs::read_dir(rx_node.downloads().join(".bolt-partial"))
        .unwrap()
        .flatten()
        .collect();
    assert!(
        leftovers.is_empty(),
        "partial dir should be clean: {leftovers:?}"
    );
}

/// Stands in for an Android SAF document: no path is ever given to the engine,
/// only a way to obtain an open handle (the platform hands over a file descriptor).
#[derive(Debug)]
struct HandleOpener(PathBuf);
impl FileOpener for HandleOpener {
    fn open(&self) -> std::io::Result<fs::File> {
        fs::File::open(&self.0)
    }
}

#[tokio::test]
async fn opener_backed_items_transfer_like_path_items() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (_h, events) = rx_node.start_receiver(false);
    let mut finished = auto_user(events, true);
    let big = pattern(2 * 1024 * 1024 + 5, 3);
    let a = write_file(tx_node.dir.path(), "hidden-uri-a", &big);
    let b = write_file(tx_node.dir.path(), "hidden-uri-b", b"tiny");
    let mk = |rel: &str, p: &Path, len: usize| {
        SendItem::from_opener(
            rel.into(),
            len as u64,
            format!("{len}-0"),
            Arc::new(HandleOpener(p.to_path_buf())),
        )
    };
    let items = finalize_items(vec![
        mk("Trip/photo.jpg", &a, big.len()),
        mk("Trip/notes/readme.txt", &b, 4),
    ])
    .unwrap();

    let summary = send(&rx_node, &tx_node, items).await.unwrap();
    assert_eq!(summary.files, 2);
    timeout(T, finished.recv()).await.unwrap().unwrap().unwrap();
    let d = rx_node.downloads().join("Trip");
    assert_eq!(fs::read(d.join("photo.jpg")).unwrap(), big);
    assert_eq!(fs::read(d.join("notes/readme.txt")).unwrap(), b"tiny");
}

#[tokio::test]
async fn probe_sees_live_receivers_and_not_dead_ones() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (_h, _events) = rx_node.start_receiver(false);
    let wait = Duration::from_secs(2);
    assert!(probe(&tx_node.ep, rx_node.addr(), Some(&rx_node.id()), wait).await);
    // wrong identity is rejected even though something answers
    assert!(!probe(&tx_node.ep, rx_node.addr(), Some(&tx_node.id()), wait).await);
    let gone = rx_node.addr();
    rx_node.ep.close();
    drop(_h);
    drop(rx_node);
    assert!(!probe(&tx_node.ep, gone, None, Duration::from_millis(800)).await);
}

#[tokio::test]
async fn declined_transfer_writes_nothing_and_reports_rejection() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (_h, events) = rx_node.start_receiver(false);
    let _finished = auto_user(events, false);
    let p = write_file(tx_node.dir.path(), "a.txt", b"secret");

    let err = send(&rx_node, &tx_node, scan(&[p]).unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, TransferError::Rejected(ref r) if r.contains("no thanks")),
        "{err}"
    );
    let any_file = walk_files(&rx_node.downloads());
    assert!(any_file.is_empty(), "nothing may be written: {any_file:?}");
}

fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk_files(&p))
            } else {
                out.push(p)
            }
        }
    }
    out
}

#[tokio::test]
async fn unanswered_offer_times_out_and_is_declined() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (_h, mut events) = rx_node.start_receiver(false); // nobody answers
    tokio::spawn(async move {
        let mut held = vec![];
        while let Some(ev) = events.recv().await {
            held.push(ev); // keep the responder alive but never answer
        }
    });
    let p = write_file(tx_node.dir.path(), "a.txt", b"x");
    let err = send(&rx_node, &tx_node, scan(&[p]).unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, TransferError::Rejected(_) | TransferError::Timeout(_)),
        "{err}"
    );
    assert!(walk_files(&rx_node.downloads()).is_empty());
}

#[tokio::test]
async fn trusted_sender_skips_prompt_only_when_enabled() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    rx_node.trust_peer(&tx_node.id());
    let (_h, mut events) = rx_node.start_receiver(true);
    let (offer_seen_tx, mut offer_seen) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(ev) = events.recv().await {
            if let IncomingEvent::OfferReceived(o) = ev {
                let _ = offer_seen_tx.send(());
                o.reject("should not be asked");
            }
        }
    });
    let p = write_file(tx_node.dir.path(), "t.txt", b"trusted");
    send(&rx_node, &tx_node, scan(&[p]).unwrap()).await.unwrap();
    assert!(
        offer_seen.try_recv().is_err(),
        "auto-accept must not prompt"
    );
    assert_eq!(
        fs::read(rx_node.downloads().join("t.txt")).unwrap(),
        b"trusted"
    );

    // Same trusted sender, auto-accept OFF -> prompted again.
    let (rx2, tx2) = (Node::new(), Node::new());
    rx2.trust_peer(&tx2.id());
    let (_h2, events2) = rx2.start_receiver(false);
    let mut fin = auto_user(events2, false);
    let p = write_file(tx2.dir.path(), "t.txt", b"x");
    assert!(matches!(
        send(&rx2, &tx2, scan(&[p]).unwrap()).await,
        Err(TransferError::Rejected(_))
    ));
    let _ = fin.try_recv();
}

#[tokio::test]
async fn wrong_identity_pin_aborts_before_sending_anything() {
    let (rx_node, tx_node, other) = (Node::new(), Node::new(), Node::new());
    let (_h, events) = rx_node.start_receiver(false);
    let _f = auto_user(events, true);
    let p = write_file(tx_node.dir.path(), "a.txt", b"x");
    let mut req = request(&rx_node, &tx_node, scan(&[p]).unwrap());
    req.expected_peer = Some(other.id());
    let (rep, _r) = ProgressReporter::new();
    let err = send_files(&tx_node.ep, req, CancellationToken::new(), rep)
        .await
        .unwrap_err();
    assert!(matches!(err, TransferError::PeerMismatch));
    assert!(walk_files(&rx_node.downloads()).is_empty());
}

#[tokio::test]
async fn pairing_succeeds_with_qr_token_and_both_sides_trust() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (h, events) = rx_node.start_receiver(false);
    let (code_tx, mut code_rx) = mpsc::unbounded_channel();
    let (mut evs, code_seen_by_responder) = (events, mpsc::unbounded_channel::<String>());
    tokio::spawn(async move {
        while let Some(ev) = evs.recv().await {
            if let IncomingEvent::PairRequested(p) = ev {
                let _ = code_seen_by_responder.0.send(p.code.clone());
                p.approve();
            }
        }
    });
    let mut responder_code = code_seen_by_responder.1;

    let uri = h.begin_pairing(Duration::from_secs(60), rx_node.addr().to_string());
    let peer = pair_with(
        &tx_node.ep,
        rx_node.addr(),
        &uri,
        &tx_node.local("phone"),
        &TransferConfig::default(),
        tx_node.trust.as_ref(),
        |c| {
            let _ = code_tx.send(c);
        },
    )
    .await
    .unwrap();

    assert_eq!(peer.id, rx_node.id());
    assert!(tx_node.trust.is_trusted(&rx_node.id()));
    assert!(rx_node.trust.is_trusted(&tx_node.id()));
    assert_eq!(
        code_rx.recv().await.unwrap(),
        responder_code.recv().await.unwrap(),
        "both screens show the same code"
    );

    // The token is single-use.
    let again = pair_with(
        &tx_node.ep,
        rx_node.addr(),
        &uri,
        &tx_node.local("phone"),
        &TransferConfig::default(),
        tx_node.trust.as_ref(),
        |_| {},
    )
    .await;
    assert!(again.is_err());
}

#[tokio::test]
async fn pairing_with_wrong_token_or_declined_does_not_trust() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (h, events) = rx_node.start_receiver(false);
    let mut evs = events;
    tokio::spawn(async move {
        while let Some(ev) = evs.recv().await {
            if let IncomingEvent::PairRequested(p) = ev {
                p.deny();
            }
        }
    });
    let mut uri = h.begin_pairing(Duration::from_secs(60), rx_node.addr().to_string());
    let good = uri.clone();
    uri.token = [9; 16];
    let r = pair_with(
        &tx_node.ep,
        rx_node.addr(),
        &uri,
        &tx_node.local("x"),
        &TransferConfig::default(),
        tx_node.trust.as_ref(),
        |_| {},
    )
    .await;
    assert!(r.is_err(), "wrong token");
    // user denies -> not trusted either
    let h2 = h.begin_pairing(Duration::from_secs(60), rx_node.addr().to_string());
    let _ = good;
    let r = pair_with(
        &tx_node.ep,
        rx_node.addr(),
        &h2,
        &tx_node.local("x"),
        &TransferConfig::default(),
        tx_node.trust.as_ref(),
        |_| {},
    )
    .await;
    assert!(matches!(r, Err(TransferError::Rejected(_))), "{r:?}");
    assert!(!tx_node.trust.is_trusted(&rx_node.id()));
    assert!(!rx_node.trust.is_trusted(&tx_node.id()));
}

#[tokio::test]
async fn pairing_without_open_session_is_refused() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (_h, events) = rx_node.start_receiver(false);
    let _f = auto_user(events, true);
    let uri = bolt_security::PairingUri {
        device_id: rx_node.id(),
        addr: rx_node.addr().to_string(),
        token: [1; 16],
        expires_unix: 0,
    };
    let r = pair_with(
        &tx_node.ep,
        rx_node.addr(),
        &uri,
        &tx_node.local("x"),
        &TransferConfig::default(),
        tx_node.trust.as_ref(),
        |_| {},
    )
    .await;
    assert!(r.is_err());
    assert!(!rx_node.trust.is_trusted(&tx_node.id()));
}

#[tokio::test]
async fn cancel_while_awaiting_approval() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (_h, mut events) = rx_node.start_receiver(false);
    tokio::spawn(async move {
        let mut held = vec![];
        while let Some(ev) = events.recv().await {
            held.push(ev);
        }
    });
    let p = write_file(tx_node.dir.path(), "a.txt", b"x");
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        c2.cancel();
    });
    let mut req = request(&rx_node, &tx_node, scan(&[p]).unwrap());
    req.config.approval_timeout = Duration::from_secs(20);
    let (rep, rx) = ProgressReporter::new();
    let err = timeout(T, send_files(&tx_node.ep, req, cancel, rep))
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(err, TransferError::Cancelled));
    assert_eq!(rx.borrow().state, TransferState::Cancelled);
}

#[tokio::test]
async fn receiver_cancel_mid_transfer_is_reported_and_nothing_is_published() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let (_h, mut events) = rx_node.start_receiver(false);
    let (fin_tx, mut fin_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(ev) = events.recv().await {
            match ev {
                IncomingEvent::OfferReceived(o) => o.accept(),
                IncomingEvent::TransferStarted {
                    progress, cancel, ..
                } => {
                    // cancel as soon as real bytes have been written
                    tokio::spawn(async move {
                        let mut p = progress;
                        while p.changed().await.is_ok() {
                            if p.borrow().bytes_done > 0 {
                                cancel.cancel();
                                break;
                            }
                        }
                    });
                }
                IncomingEvent::TransferFinished { result, .. } => {
                    let _ = fin_tx.send(result.is_ok());
                }
                _ => {}
            }
        }
    });
    let data = pattern(200 * 1024 * 1024, 3);
    let p = write_file(tx_node.dir.path(), "huge.bin", &data);
    let items = scan(&[p]).unwrap();
    let err = send(&rx_node, &tx_node, items.clone()).await.unwrap_err();
    assert!(err.is_cancel(), "{err}");
    assert_eq!(timeout(T, fin_rx.recv()).await.unwrap(), Some(false));
    assert!(
        !rx_node.downloads().join("huge.bin").exists(),
        "an incomplete file must never appear as final"
    );
}

#[tokio::test]
async fn resume_from_seeded_partial_sends_only_the_remainder() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let data = pattern(5 * 1024 * 1024 + 123, 11);
    let p = write_file(tx_node.dir.path(), "movie.mkv", &data);
    let items = scan(&[p]).unwrap();

    // Simulate an interrupted earlier attempt: first 2 MiB landed in the partial file.
    let st = Staging::new(&rx_node.downloads()).unwrap();
    let fo = FileOffer {
        index: 0,
        path: "movie.mkv".into(),
        size: items[0].size,
        file_id: items[0].file_id.clone(),
    };
    let crate::fsx::Prepared::Partial { file, .. } = st.open(&tx_node.id(), &fo).unwrap() else {
        panic!()
    };
    use std::io::Write;
    let mut file = file;
    file.write_all(&data[..2 * 1024 * 1024]).unwrap();
    drop(file);

    let (_h, events) = rx_node.start_receiver(false);
    let mut fin = auto_user(events, true);
    let (rep, prog) = ProgressReporter::new();
    let s = timeout(
        T,
        send_files(
            &tx_node.ep,
            request(&rx_node, &tx_node, items),
            CancellationToken::new(),
            rep,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(s.bytes, data.len() as u64);
    let _ = fin.recv().await;
    assert_eq!(
        fs::read(rx_node.downloads().join("movie.mkv")).unwrap(),
        data,
        "resumed file must be byte-identical"
    );
    assert_eq!(prog.borrow().bytes_done, data.len() as u64);
}

#[tokio::test]
async fn corrupted_partial_is_caught_by_final_hash_and_never_published() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    let data = pattern(2 * 1024 * 1024, 5);
    let p = write_file(tx_node.dir.path(), "doc.bin", &data);
    let items = scan(&[p]).unwrap();

    let st = Staging::new(&rx_node.downloads()).unwrap();
    let fo = FileOffer {
        index: 0,
        path: "doc.bin".into(),
        size: items[0].size,
        file_id: items[0].file_id.clone(),
    };
    let crate::fsx::Prepared::Partial { mut file, .. } = st.open(&tx_node.id(), &fo).unwrap()
    else {
        panic!()
    };
    use std::io::Write;
    let mut bad = data[..1024 * 1024].to_vec();
    bad[100] ^= 0xFF; // disk corruption in the stored prefix
    file.write_all(&bad).unwrap();
    drop(file);

    let (_h, events) = rx_node.start_receiver(false);
    let _fin = auto_user(events, true);
    let err = send(&rx_node, &tx_node, items).await.unwrap_err();
    assert!(matches!(err, TransferError::Integrity(_)), "{err}");
    assert!(!rx_node.downloads().join("doc.bin").exists());
}

/// A hostile peer speaking raw protocol.
async fn raw_client(
    from: &Node,
    to: &Node,
) -> (quinn::Connection, quinn::SendStream, quinn::RecvStream) {
    let conn = from
        .ep
        .endpoint
        .connect(to.addr(), "bolt.local")
        .unwrap()
        .await
        .unwrap();
    let (mut tx, mut rx) = conn.open_bi().await.unwrap();
    write_message(
        &mut tx,
        &Message::Hello(bolt_protocol::Hello {
            protocol: PROTOCOL_VERSION,
            device_name: "evil".into(),
            device_kind: DeviceKind::Unknown,
            max_chunk_bytes: 1,
        }),
    )
    .await
    .unwrap();
    assert!(matches!(
        read_message(&mut rx).await.unwrap(),
        Message::Hello(_)
    ));
    (conn, tx, rx)
}

#[tokio::test]
async fn hostile_offers_are_rejected_without_touching_disk() {
    let (rx_node, evil) = (Node::new(), Node::new());
    let (_h, events) = rx_node.start_receiver(false);
    let _f = auto_user(events, true);

    for bad_path in [
        "../../escape.txt",
        "/abs/path",
        "C:\\Windows\\evil.dll",
        "ok/../../x",
        "CON",
        "a/./b",
    ] {
        let (conn, mut tx, mut rx) = raw_client(&evil, &rx_node).await;
        let offer = TransferOffer {
            transfer_id: "00112233445566778899aabbccddeeff".into(),
            files: vec![FileOffer {
                index: 0,
                path: bad_path.into(),
                size: 1,
                file_id: "x".into(),
            }],
        };
        write_message(&mut tx, &Message::TransferOffer(offer))
            .await
            .unwrap();
        let reply = timeout(T, read_message(&mut rx)).await.unwrap();
        assert!(
            matches!(reply, Ok(Message::TransferReject(_))),
            "{bad_path}: {reply:?}"
        );
        conn.close(0u32.into(), b"");
    }
    assert!(
        walk_files(&rx_node.downloads()).is_empty()
            || walk_files(&rx_node.downloads())
                .iter()
                .all(|p| p.to_string_lossy().contains(".bolt-partial"))
    );
    assert!(!rx_node.dir.path().join("escape.txt").exists());
}

#[tokio::test]
async fn wrong_hash_from_sender_is_rejected_and_file_not_published() {
    let (rx_node, evil) = (Node::new(), Node::new());
    let (_h, events) = rx_node.start_receiver(false);
    let mut fin = auto_user(events, true);

    let (conn, mut tx, mut rx) = raw_client(&evil, &rx_node).await;
    let offer = TransferOffer {
        transfer_id: "00112233445566778899aabbccddeeff".into(),
        files: vec![FileOffer {
            index: 0,
            path: "x.bin".into(),
            size: 4,
            file_id: "x".into(),
        }],
    };
    write_message(&mut tx, &Message::TransferOffer(offer))
        .await
        .unwrap();
    assert!(matches!(
        read_message(&mut rx).await.unwrap(),
        Message::TransferAccept(_)
    ));
    write_message(
        &mut tx,
        &Message::FileStart(FileStart {
            index: 0,
            offset: 0,
            length: 4,
        }),
    )
    .await
    .unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut tx, b"abcd")
        .await
        .unwrap();
    write_message(
        &mut tx,
        &Message::FileComplete(FileComplete {
            index: 0,
            blake3: "00".repeat(32),
        }),
    )
    .await
    .unwrap();
    match timeout(T, read_message(&mut rx)).await.unwrap().unwrap() {
        Message::FileAck(a) => assert!(!a.ok),
        other => panic!("unexpected {other:?}"),
    }
    assert!(timeout(T, fin.recv()).await.unwrap().unwrap().is_err());
    assert!(!rx_node.downloads().join("x.bin").exists());
    conn.close(0u32.into(), b"");
}

#[tokio::test]
async fn data_beyond_announced_length_cannot_overflow_the_file() {
    let (rx_node, evil) = (Node::new(), Node::new());
    let (_h, events) = rx_node.start_receiver(false);
    let mut fin = auto_user(events, true);

    let (conn, mut tx, mut rx) = raw_client(&evil, &rx_node).await;
    let offer = TransferOffer {
        transfer_id: "00112233445566778899aabbccddeeff".into(),
        files: vec![FileOffer {
            index: 0,
            path: "y.bin".into(),
            size: 4,
            file_id: "x".into(),
        }],
    };
    write_message(&mut tx, &Message::TransferOffer(offer))
        .await
        .unwrap();
    let _ = read_message(&mut rx).await.unwrap();
    // Claims 4 bytes in FILE_START but sends 4MB of junk after the frame.
    write_message(
        &mut tx,
        &Message::FileStart(FileStart {
            index: 0,
            offset: 0,
            length: 4,
        }),
    )
    .await
    .unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut tx, &vec![7u8; 4 * 1024 * 1024])
        .await
        .unwrap();
    let r = timeout(T, fin.recv()).await.unwrap().unwrap();
    assert!(
        r.is_err(),
        "receiver must fail the transfer, not accept surplus bytes"
    );
    assert!(!rx_node.downloads().join("y.bin").exists());
    conn.close(0u32.into(), b"");
}

#[tokio::test]
async fn concurrent_transfers_are_isolated() {
    let rx_node = Node::new();
    let (_h, events) = rx_node.start_receiver(false);
    let mut fin = auto_user(events, true);
    let senders: Vec<Node> = (0..3).map(|_| Node::new()).collect();
    let mut tasks = vec![];
    for (i, s) in senders.iter().enumerate() {
        let data = pattern(2 * 1024 * 1024 + i, i as u8);
        let p = write_file(s.dir.path(), "same-name.bin", &data);
        let items = scan(&[p]).unwrap();
        let (ep, req) = (s.ep.clone(), request(&rx_node, s, items));
        tasks.push(tokio::spawn(async move {
            let (rep, _rx) = ProgressReporter::new();
            send_files(&ep, req, CancellationToken::new(), rep)
                .await
                .map(|_| data)
        }));
    }
    let mut sent = vec![];
    for t in tasks {
        sent.push(timeout(T, t).await.unwrap().unwrap().unwrap());
    }
    for _ in 0..3 {
        timeout(T, fin.recv()).await.unwrap().unwrap().unwrap();
    }
    // Three identically named files from different senders: all kept, none overwritten.
    let mut received: Vec<Vec<u8>> = walk_files(&rx_node.downloads())
        .into_iter()
        .filter(|p| !p.to_string_lossy().contains(".bolt-partial"))
        .map(|p| fs::read(p).unwrap())
        .collect();
    received.sort();
    sent.sort();
    assert_eq!(received, sent);
}

#[tokio::test]
async fn download_dir_that_is_a_file_fails_cleanly() {
    let (rx_node, tx_node) = (Node::new(), Node::new());
    fs::write(rx_node.downloads(), b"i am a file, not a directory").unwrap_or_else(|_| {
        fs::create_dir_all(rx_node.dir.path()).unwrap();
        fs::write(rx_node.downloads(), b"x").unwrap();
    });
    let (_h, events) = rx_node.start_receiver(false);
    let _f = auto_user(events, true);
    let p = write_file(tx_node.dir.path(), "a.txt", b"x");
    let r = send(&rx_node, &tx_node, scan(&[p]).unwrap()).await;
    assert!(
        r.is_err(),
        "must not report success when the destination is unusable"
    );
}

/// Raw QUIC stream throughput with no disk and no hashing: isolates the transport.
/// Run: cargo test -p bolt-transfer-core --release -- --ignored --nocapture raw_quic
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn raw_quic_throughput() {
    let (a, b) = (Node::new(), Node::new());
    let b_ep = b.ep.clone();
    let recv = tokio::spawn(async move {
        let conn = b_ep.endpoint.accept().await.unwrap().await.unwrap();
        let (_tx, mut rx) = conn.accept_bi().await.unwrap();
        let mut buf = vec![0u8; 256 * 1024];
        let mut total = 0u64;
        while let Some(n) = rx.read(&mut buf).await.unwrap() {
            total += n as u64;
        }
        total
    });
    let conn =
        a.ep.endpoint
            .connect(b.addr(), "bolt.local")
            .unwrap()
            .await
            .unwrap();
    let (mut tx, _rx) = conn.open_bi().await.unwrap();
    let chunk = vec![0u8; 256 * 1024];
    let t = std::time::Instant::now();
    for _ in 0..(1024 * 4) {
        tx.write_all(&chunk).await.unwrap();
    }
    tx.finish().unwrap();
    let total = recv.await.unwrap();
    let s = t.elapsed().as_secs_f64();
    println!(
        "raw QUIC loopback: {} MiB in {s:.2}s = {:.0} MB/s",
        total >> 20,
        total as f64 / 1e6 / s
    );
}

#[tokio::test]
async fn probe_peer_learns_identity_without_offering_anything() {
    let rx = Node::new();
    let tx = Node::new();
    let (_h, mut events) = rx.start_receiver(false);
    let peer = timeout(
        T,
        probe_peer(
            &tx.ep,
            rx.addr(),
            &tx.local("prober"),
            &TransferConfig::default(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(peer.id, rx.id());
    assert_eq!(peer.name, "receiver");
    // A probe must never surface an offer or transfer to the receiving user.
    assert!(timeout(Duration::from_millis(300), events.recv())
        .await
        .is_err());
}

#[tokio::test]
async fn probe_peer_times_out_when_nothing_listens() {
    let tx = Node::new();
    let dead = Node::new(); // bound but no receiver running
    let cfg = TransferConfig {
        connect_timeout: Duration::from_millis(500),
        handshake_timeout: Duration::from_millis(500),
        ..Default::default()
    };
    let r = timeout(T, probe_peer(&tx.ep, dead.addr(), &tx.local("p"), &cfg))
        .await
        .unwrap();
    assert!(r.is_err());
}
