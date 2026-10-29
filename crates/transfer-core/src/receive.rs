use crate::{
    endpoint::{peer_id, FlashEndpoint},
    error::TransferError,
    fsx::{Prepared, Probe, Staging},
    progress::{Progress, ProgressReporter, TransferState},
    send::{hello, LocalInfo, PeerInfo},
    stats::{PathInfo, StageTimes},
    TransferConfig, CLOSE_CANCELLED, CLOSE_DONE, CLOSE_PROTOCOL,
};
use bolt_protocol::*;
use bolt_security::{
    pairing, verification_code, PairingSession, PairingUri, TrustStore, TrustedDevice,
};
use quinn::{Connection, Incoming, RecvStream, SendStream, VarInt};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    sync::{mpsc, oneshot, watch, Semaphore},
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

const MAX_CONCURRENT_CONNECTIONS: usize = 16;
const PAIR_APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);
const STALE_PARTIAL_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    pub download_dir: PathBuf,
    pub local: LocalInfo,
    /// Skip the confirmation prompt for devices the user explicitly paired.
    pub auto_accept_trusted: bool,
    pub max_concurrent_transfers: usize,
    pub transfer: TransferConfig,
}

#[derive(Debug)]
pub enum OfferDecision {
    Accept { dest_dir: Option<PathBuf> },
    Reject { reason: String },
}

/// An incoming transfer waiting for the user. Dropping it declines.
pub struct IncomingOffer {
    pub peer: PeerInfo,
    pub trusted: bool,
    pub offer: TransferOffer,
    pub total_bytes: u64,
    pub free_space: Option<u64>,
    respond: oneshot::Sender<OfferDecision>,
}

impl IncomingOffer {
    pub fn accept(self) {
        let _ = self.respond.send(OfferDecision::Accept { dest_dir: None });
    }
    pub fn accept_into(self, dir: PathBuf) {
        let _ = self.respond.send(OfferDecision::Accept {
            dest_dir: Some(dir),
        });
    }
    pub fn reject(self, reason: impl Into<String>) {
        let _ = self.respond.send(OfferDecision::Reject {
            reason: reason.into(),
        });
    }
}

/// A pairing request waiting for the user to compare codes. Dropping it denies.
pub struct IncomingPair {
    pub peer: PeerInfo,
    /// Six digits both screens must show before the user approves.
    pub code: String,
    respond: oneshot::Sender<bool>,
}

impl IncomingPair {
    pub fn approve(self) {
        let _ = self.respond.send(true);
    }
    pub fn deny(self) {
        let _ = self.respond.send(false);
    }
}

#[derive(Debug, Clone)]
pub struct ReceivedSummary {
    pub transfer_id: String,
    pub files: u32,
    pub bytes: u64,
    pub saved_paths: Vec<PathBuf>,
    pub elapsed: Duration,
    /// Where the time went (network reads, file writes, hashing, fsync, publish).
    pub timings: StageTimes,
    /// QUIC's own view of the path (loss, RTT, congestion window, datagram size).
    pub path: PathInfo,
}

pub enum IncomingEvent {
    OfferReceived(IncomingOffer),
    PairRequested(IncomingPair),
    TransferStarted {
        transfer_id: String,
        peer: PeerInfo,
        progress: watch::Receiver<Progress>,
        cancel: CancellationToken,
    },
    TransferFinished {
        transfer_id: String,
        peer: PeerInfo,
        result: Result<ReceivedSummary, TransferError>,
    },
}

struct Inner {
    ep: FlashEndpoint,
    cfg: RwLock<ReceiverConfig>,
    trust: Arc<dyn TrustStore>,
    pairing: Mutex<Option<PairingSession>>,
    conn_slots: Semaphore,
    transfer_slots: Semaphore,
}

pub struct Receiver;

#[derive(Clone)]
pub struct ReceiverHandle {
    inner: Arc<Inner>,
    shutdown: CancellationToken,
}

impl Receiver {
    /// Start accepting connections on `ep`. UI-facing events arrive on the returned channel.
    pub fn start(
        ep: FlashEndpoint,
        cfg: ReceiverConfig,
        trust: Arc<dyn TrustStore>,
    ) -> (ReceiverHandle, mpsc::Receiver<IncomingEvent>) {
        let (tx, rx) = mpsc::channel(64);
        let slots = cfg.max_concurrent_transfers.max(1);
        if let Ok(st) = Staging::new(&cfg.download_dir) {
            let purged = st.purge_stale(STALE_PARTIAL_AGE);
            if purged > 0 {
                info!(purged, "removed stale partial files");
            }
        }
        let inner = Arc::new(Inner {
            ep,
            cfg: RwLock::new(cfg),
            trust,
            pairing: Mutex::new(None),
            conn_slots: Semaphore::new(MAX_CONCURRENT_CONNECTIONS),
            transfer_slots: Semaphore::new(slots),
        });
        let shutdown = CancellationToken::new();
        tokio::spawn(accept_loop(inner.clone(), tx, shutdown.clone()));
        (ReceiverHandle { inner, shutdown }, rx)
    }
}

impl Inner {
    pub(crate) fn transfer_slots_ref(&self) -> &Semaphore {
        &self.transfer_slots
    }
}

impl ReceiverHandle {
    /// Open a short-lived pairing window and return the contents for the QR code.
    /// `advertised_addr` is the `ip:port` a peer should dial (not `0.0.0.0`).
    pub fn begin_pairing(&self, ttl: Duration, advertised_addr: String) -> PairingUri {
        let session = PairingSession::new(ttl);
        let token = *session.token();
        *self.inner.pairing.lock().unwrap_or_else(|p| p.into_inner()) = Some(session);
        let expires_unix = (SystemTime::now() + ttl)
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        PairingUri {
            device_id: self.inner.ep.device_id().clone(),
            addr: advertised_addr,
            token,
            expires_unix,
        }
    }

    pub fn end_pairing(&self) {
        *self.inner.pairing.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    pub fn set_download_dir(&self, dir: PathBuf) {
        self.inner
            .cfg
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .download_dir = dir;
    }

    pub fn set_auto_accept_trusted(&self, on: bool) {
        self.inner
            .cfg
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .auto_accept_trusted = on;
    }

    pub fn set_device_name(&self, name: String) {
        self.inner
            .cfg
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .local
            .name = name;
    }

    pub fn shutdown(&self) {
        self.shutdown.cancel();
        self.inner.ep.close();
    }
}

async fn accept_loop(
    inner: Arc<Inner>,
    events: mpsc::Sender<IncomingEvent>,
    shutdown: CancellationToken,
) {
    loop {
        let incoming = tokio::select! {
            _ = shutdown.cancelled() => break,
            i = inner.ep.endpoint.accept() => match i { Some(i) => i, None => break },
        };
        let Ok(permit) = inner.conn_slots.try_acquire() else {
            incoming.refuse();
            continue;
        };
        permit.forget();
        let inner = inner.clone();
        let events = events.clone();
        tokio::spawn(async move {
            serve(&inner, incoming, &events).await;
            inner.conn_slots.add_permits(1);
        });
    }
    debug!("accept loop ended");
}

async fn serve(inner: &Arc<Inner>, incoming: Incoming, events: &mpsc::Sender<IncomingEvent>) {
    let conn = match incoming.await {
        Ok(c) => c,
        Err(e) => {
            debug!(error = %e, "handshake failed");
            return;
        }
    };
    if let Err(e) = serve_conn(inner, &conn, events).await {
        let e = e.refine(&conn);
        debug!(error = %e, "connection ended with error");
        // Closing immediately would discard the rejection/error reply still in flight;
        // give the peer a moment to read it (it closes first in the normal case).
        if conn.close_reason().is_none() {
            let _ = timeout(Duration::from_secs(2), conn.closed()).await;
        }
        let code = if matches!(e, TransferError::Protocol(_)) {
            CLOSE_PROTOCOL
        } else {
            CLOSE_DONE
        };
        conn.close(VarInt::from_u32(code), b"error");
    }
}

async fn serve_conn(
    inner: &Arc<Inner>,
    conn: &Connection,
    events: &mpsc::Sender<IncomingEvent>,
) -> Result<(), TransferError> {
    let cfg = inner.cfg.read().unwrap_or_else(|p| p.into_inner()).clone();
    let id = peer_id(conn)
        .ok_or_else(|| TransferError::Protocol("peer presented no certificate".into()))?;
    let (mut tx, mut rx) = timeout(cfg.transfer.handshake_timeout, conn.accept_bi())
        .await
        .map_err(|_| TransferError::Timeout("waiting for peer to open a stream"))??;

    let Message::Hello(h) = timeout(cfg.transfer.handshake_timeout, read_message(&mut rx))
        .await
        .map_err(|_| TransferError::Timeout("peer hello"))??
    else {
        return Err(TransferError::Protocol("expected HELLO".into()));
    };
    h.check_version()?;
    let peer = PeerInfo {
        id,
        name: sanitize_display_name(&h.device_name),
        kind: h.device_kind,
    };
    write_message(&mut tx, &hello(&cfg.local, &cfg.transfer)).await?;

    let first = timeout(cfg.transfer.handshake_timeout, read_message(&mut rx))
        .await
        .map_err(|_| TransferError::Timeout("first request"))??;
    match first {
        Message::PairRequest(req) => handle_pair(inner, conn, &mut tx, peer, req, events).await,
        Message::TransferOffer(offer) => {
            handle_offer(inner, &cfg, conn, tx, rx, peer, offer, events).await
        }
        Message::LinkTest(req) => {
            crate::linktest::serve(inner.transfer_slots_ref(), conn, tx, rx, req).await
        }
        _ => Err(TransferError::Protocol(
            "expected PAIR_REQUEST, TRANSFER_OFFER or LINK_TEST".into(),
        )),
    }
}

/// Peer-supplied names are display text only: strip control characters, cap length.
fn sanitize_display_name(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_control()).take(64).collect();
    if t.trim().is_empty() {
        "Unknown device".into()
    } else {
        t
    }
}

async fn handle_pair(
    inner: &Arc<Inner>,
    conn: &Connection,
    tx: &mut SendStream,
    peer: PeerInfo,
    req: PairRequest,
    events: &mpsc::Sender<IncomingEvent>,
) -> Result<(), TransferError> {
    let local_id = inner.ep.device_id().clone();
    let verdict = {
        let mut guard = inner.pairing.lock().unwrap_or_else(|p| p.into_inner());
        match guard.as_mut() {
            None => Err(pairing::PairError::Expired),
            Some(s) => {
                let token = *s.token();
                s.verify(&peer.id, &local_id, &req.proof).map(|_| token)
            }
        }
    };
    let token = match verdict {
        Ok(t) => t,
        Err(e) => {
            warn!(peer = %peer.id.short(), "pairing attempt rejected: {e}");
            write_message(
                tx,
                &Message::PairResponse(PairResponse {
                    approved: false,
                    reason: Some(e.to_string()),
                }),
            )
            .await?;
            let _ = tx.finish();
            let _ = timeout(Duration::from_secs(2), conn.closed()).await;
            return Ok(());
        }
    };

    let code = verification_code(&peer.id, &local_id, &token);
    let (respond, answer) = oneshot::channel();
    events
        .send(IncomingEvent::PairRequested(IncomingPair {
            peer: peer.clone(),
            code,
            respond,
        }))
        .await
        .map_err(|_| TransferError::Setup("no UI is listening for pairing requests".into()))?;
    let approved = matches!(timeout(PAIR_APPROVAL_TIMEOUT, answer).await, Ok(Ok(true)));

    if approved {
        inner
            .trust
            .trust(TrustedDevice {
                id: peer.id.clone(),
                name: peer.name.clone(),
                paired_at_unix: now_unix(),
                last_seen_unix: Some(now_unix()),
            })
            .map_err(|e| TransferError::Setup(e.to_string()))?;
        info!(peer = %peer.id.short(), "device paired");
    }
    let reason = (!approved).then(|| "pairing was declined".to_string());
    write_message(
        tx,
        &Message::PairResponse(PairResponse { approved, reason }),
    )
    .await?;
    let _ = tx.finish();
    let _ = timeout(Duration::from_secs(5), conn.closed()).await;
    Ok(())
}

pub(crate) fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
async fn handle_offer(
    inner: &Arc<Inner>,
    cfg: &ReceiverConfig,
    conn: &Connection,
    mut tx: SendStream,
    mut rx: RecvStream,
    peer: PeerInfo,
    offer: TransferOffer,
    events: &mpsc::Sender<IncomingEvent>,
) -> Result<(), TransferError> {
    async fn reject(tx: &mut SendStream, reason: &str) -> Result<(), TransferError> {
        write_message(
            tx,
            &Message::TransferReject(TransferReject {
                reason: reason.into(),
            }),
        )
        .await?;
        let _ = tx.finish();
        Ok(())
    }

    if let Err(e) = offer.validate() {
        reject(&mut tx, "The offer was invalid").await?;
        return Err(e.into());
    }
    let trusted = inner.trust.is_trusted(&peer.id);
    let total = offer.total_bytes();

    let Ok(_slot) = inner.transfer_slots.try_acquire() else {
        reject(&mut tx, "The receiver is busy with other transfers").await?;
        return Err(TransferError::Rejected("busy".into()));
    };

    let staging = Staging::new(&cfg.download_dir)?;
    let free = staging.available_space().ok();

    // Decide: only explicitly trusted devices may skip the prompt, and only if the user enabled it.
    let decision = if trusted && cfg.auto_accept_trusted {
        OfferDecision::Accept { dest_dir: None }
    } else {
        let (respond, answer) = oneshot::channel();
        let ev = IncomingEvent::OfferReceived(IncomingOffer {
            peer: peer.clone(),
            trusted,
            offer: offer.clone(),
            total_bytes: total,
            free_space: free,
            respond,
        });
        if events.send(ev).await.is_err() {
            reject(&mut tx, "The receiver is not ready").await?;
            return Err(TransferError::Setup(
                "no UI is listening for incoming offers".into(),
            ));
        }
        match timeout(cfg.transfer.approval_timeout, answer).await {
            Ok(Ok(d)) => d,
            _ => OfferDecision::Reject {
                reason: "No response from the receiving device".into(),
            },
        }
    };

    let dest_dir = match decision {
        OfferDecision::Reject { reason } => {
            reject(&mut tx, &reason).await?;
            let _ = timeout(Duration::from_secs(2), conn.closed()).await;
            return Ok(());
        }
        OfferDecision::Accept { dest_dir } => dest_dir.unwrap_or_else(|| cfg.download_dir.clone()),
    };
    let staging = Staging::new(&dest_dir)?;

    // Plan resume points (read-only) and check space for what still has to arrive.
    let mut resume = Vec::with_capacity(offer.files.len());
    let mut needed = 0u64;
    for f in &offer.files {
        let (offset, complete) = match staging.probe(&peer.id, f)? {
            Probe::Complete => (f.size, true),
            Probe::Resume { offset } => (offset, false),
            Probe::Fresh => (0, false),
        };
        needed = needed.saturating_add(f.size - offset);
        resume.push(ResumePoint {
            index: f.index,
            offset,
            complete,
        });
    }
    if let Ok(avail) = staging.available_space() {
        if avail < needed {
            reject(&mut tx, "Not enough free disk space").await?;
            return Err(TransferError::DiskFull {
                need: needed,
                available: avail,
            });
        }
    }

    let (mut reporter, progress_rx) = ProgressReporter::new();
    reporter.init(&offer.transfer_id, total, offer.files.len() as u32);
    let cancel = CancellationToken::new();
    let _ = events
        .send(IncomingEvent::TransferStarted {
            transfer_id: offer.transfer_id.clone(),
            peer: peer.clone(),
            progress: progress_rx,
            cancel: cancel.clone(),
        })
        .await;

    // The accept reply is inside the cancellable section too: if the peer vanished (or the write
    // fails) the user can still cancel, and the failure is reported like any other. It used to
    // return early, leaving the UI stuck on "Connecting" with a Cancel button that did nothing.
    let started = Instant::now();
    let result = tokio::select! {
        _ = cancel.cancelled() => {
            conn.close(VarInt::from_u32(CLOSE_CANCELLED), b"cancelled");
            Err(TransferError::Cancelled)
        }
        r = async {
            write_message(
                &mut tx,
                &Message::TransferAccept(TransferAccept {
                    transfer_id: offer.transfer_id.clone(),
                    resume: resume.clone(),
                }),
            )
            .await
            .map_err(TransferError::from)?;
            reporter.set_bytes(resume.iter().map(|r| r.offset).sum());
            reporter.set_state(TransferState::Transferring);
            receive_files(cfg, &staging, &peer, &offer, &resume, &mut tx, &mut rx, &mut reporter, conn, started).await
        } => r.map_err(|e| e.refine(conn)),
    };

    reporter.set_state(match &result {
        Ok(_) => TransferState::Completed,
        Err(e) if e.is_cancel() => TransferState::Cancelled,
        Err(_) => TransferState::Failed,
    });
    if let Err(e) = &result {
        // Tell the sender why (best effort); keep partial files so the sender can resume.
        if !e.is_cancel() && !matches!(e, TransferError::Connection(_) | TransferError::Io(_)) {
            let code = match e {
                TransferError::Integrity(_) => ErrorCode::Integrity,
                TransferError::DiskFull { .. } => ErrorCode::DiskFull,
                TransferError::Protocol(_) => ErrorCode::Protocol,
                _ => ErrorCode::Internal,
            };
            let _ = write_message(
                &mut tx,
                &Message::TransferError(bolt_protocol::TransferError {
                    code,
                    message: e.to_string(),
                }),
            )
            .await;
        }
    }
    let report = match &result {
        Ok(s) => Ok(s.clone()),
        Err(e) => Err(TransferError::Setup(e.to_string())),
    };
    let _ = events
        .send(IncomingEvent::TransferFinished {
            transfer_id: offer.transfer_id.clone(),
            peer,
            result: report,
        })
        .await;
    result.map(|_| ())
}

#[allow(clippy::too_many_arguments)]
async fn receive_files(
    cfg: &ReceiverConfig,
    staging: &Staging,
    peer: &PeerInfo,
    offer: &TransferOffer,
    resume: &[ResumePoint],
    tx: &mut SendStream,
    rx: &mut RecvStream,
    reporter: &mut ProgressReporter,
    conn: &Connection,
    started: Instant,
) -> Result<ReceivedSummary, TransferError> {
    let mut buf = vec![0u8; cfg.transfer.clamped_chunk()];
    let mut saved = Vec::with_capacity(offer.files.len());
    let mut keys = Vec::with_capacity(offer.files.len());
    let mut st = StageTimes::default();

    for (fo, rp) in offer.files.iter().zip(resume) {
        reporter.begin_file(&fo.path);
        keys.push(Staging::key(&peer.id, fo));
        if rp.complete {
            reporter.file_done();
            continue;
        }
        let Message::FileStart(fs) = read_message(rx).await? else {
            return Err(TransferError::Protocol("expected FILE_START".into()));
        };
        if fs.index != fo.index || fs.offset != rp.offset || fs.length != fo.size - rp.offset {
            return Err(TransferError::Protocol(
                "FILE_START does not match the accepted plan".into(),
            ));
        }
        let Prepared::Partial { file, offset, key } = staging.open(&peer.id, fo)? else {
            return Err(TransferError::Protocol(
                "file state changed after acceptance".into(),
            ));
        };
        if offset != rp.offset {
            return Err(TransferError::Protocol(
                "partial file changed after acceptance".into(),
            ));
        }

        let mut hasher = blake3::Hasher::new();
        if offset > 0 {
            let st = staging.clone();
            let k = key.clone();
            hasher = tokio::task::spawn_blocking(move || {
                st.hash_prefix(&k, offset, &mut hasher).map(|_| hasher)
            })
            .await
            .map_err(|e| TransferError::Setup(e.to_string()))??;
        }

        let mut out = File::from_std(file);
        let mut remaining = fs.length;
        while remaining > 0 {
            let want = buf.len().min(remaining as usize);
            // Fill the whole chunk buffer before touching disk or the hasher. `read` returns one packet's
            // worth (~1.4 KiB) at a time; doing a file write + hash per packet costs far more CPU than the
            // payload is worth, which a phone's slower cores feel long before the radio is the limit.
            let mut filled = 0usize;
            while filled < want {
                let t = Instant::now();
                let got = rx
                    .read(&mut buf[filled..want])
                    .await
                    .map_err(TransferError::from)?;
                st.net += t.elapsed();
                match got {
                    Some(n) => {
                        filled += n;
                        st.net_calls += 1;
                        st.bytes += n as u64;
                    }
                    None if filled > 0 => break,
                    None => {
                        return Err(TransferError::Connection(
                            "stream ended before the file was complete".into(),
                        ))
                    }
                }
            }
            let n = filled;
            let t = Instant::now();
            if let Err(e) = out.write_all(&buf[..n]).await {
                return Err(disk_error(e, staging, remaining));
            }
            st.disk += t.elapsed();
            let t = Instant::now();
            hasher.update(&buf[..n]);
            st.hash += t.elapsed();
            remaining -= n as u64;
            reporter.add_bytes(n as u64);
        }

        let Message::FileComplete(fc) = read_message(rx).await? else {
            return Err(TransferError::Protocol("expected FILE_COMPLETE".into()));
        };
        if fc.index != fo.index {
            return Err(TransferError::Protocol(
                "FILE_COMPLETE index mismatch".into(),
            ));
        }
        let t = Instant::now();
        out.flush().await?;
        out.sync_data().await?;
        drop(out);
        st.sync += t.elapsed();

        let digest = hasher.finalize().to_hex().to_string();
        if digest != fc.blake3 {
            staging.discard(&key);
            write_message(
                tx,
                &Message::FileAck(FileAck {
                    index: fo.index,
                    ok: false,
                    error: Some(format!("checksum mismatch for {}", fo.path)),
                }),
            )
            .await?;
            return Err(TransferError::Integrity(fo.path.clone()));
        }
        let t = Instant::now();
        let final_path = staging
            .finalize(&key, &fo.path, fo.size)
            .map_err(|e| disk_error(e, staging, 0))?;
        st.finalize += t.elapsed();
        saved.push(final_path);
        write_message(
            tx,
            &Message::FileAck(FileAck {
                index: fo.index,
                ok: true,
                error: None,
            }),
        )
        .await?;
        reporter.file_done();
    }

    match read_message(rx).await? {
        Message::TransferComplete => {}
        _ => return Err(TransferError::Protocol("expected TRANSFER_COMPLETE".into())),
    }
    staging.clear_done(&keys);
    write_message(tx, &Message::TransferComplete).await?;
    let _ = tx.finish();
    // Give the sender time to read the final message before the connection is torn down.
    let _ = timeout(Duration::from_secs(5), conn.closed()).await;

    let path = PathInfo::of(conn);
    tracing::info!(target: "bolt::timing", role = "receive", bytes = offer.total_bytes(), "{st} | {path}");
    Ok(ReceivedSummary {
        transfer_id: offer.transfer_id.clone(),
        files: offer.files.len() as u32,
        bytes: offer.total_bytes(),
        saved_paths: saved,
        elapsed: started.elapsed(),
        timings: st,
        path,
    })
}

fn disk_error(e: std::io::Error, staging: &Staging, need: u64) -> TransferError {
    if e.kind() == std::io::ErrorKind::StorageFull {
        TransferError::DiskFull {
            need,
            available: staging.available_space().unwrap_or(0),
        }
    } else {
        TransferError::Io(e)
    }
}
