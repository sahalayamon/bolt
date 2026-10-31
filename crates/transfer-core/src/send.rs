use crate::{
    config::TransferConfig,
    endpoint::{peer_id, FlashEndpoint},
    error::TransferError,
    progress::{ProgressReporter, TransferState},
    scan::SendItem,
    stats::{PathInfo, StageTimes},
    CLOSE_CANCELLED, CLOSE_DONE, CLOSE_PROTOCOL,
};
use bolt_protocol::*;
use bolt_security::DeviceId;
use quinn::{Connection, RecvStream, SendStream, VarInt};
use rand::RngCore;
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
    time::timeout,
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct LocalInfo {
    pub name: String,
    pub kind: DeviceKind,
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
}

pub struct SendRequest {
    pub addr: SocketAddr,
    /// If set, the connection is dropped unless the peer's certificate matches.
    pub expected_peer: Option<DeviceId>,
    pub local: LocalInfo,
    pub items: Vec<SendItem>,
    pub config: TransferConfig,
}

#[derive(Debug, Clone)]
pub struct TransferSummary {
    pub transfer_id: String,
    pub peer: PeerInfo,
    pub files: u32,
    pub bytes: u64,
    pub elapsed: Duration,
    /// Where the time went (file reads, hashing, waiting on the network, final ack).
    pub timings: StageTimes,
    /// QUIC's own view of the path (loss, RTT, congestion window, datagram size).
    pub path: PathInfo,
}

pub(crate) fn new_transfer_id() -> String {
    let mut b = [0u8; 16];
    rand::rng().fill_bytes(&mut b);
    hex::encode(b)
}

pub(crate) fn hello(local: &LocalInfo, cfg: &TransferConfig) -> Message {
    Message::Hello(Hello {
        protocol: PROTOCOL_VERSION,
        device_name: local.name.chars().take(64).collect(),
        device_kind: local.kind,
        max_chunk_bytes: cfg.clamped_chunk() as u32,
    })
}

/// Send `req.items` to a peer. Completes only after the receiver has verified
/// and finalised every file.
pub async fn send_files(
    ep: &FlashEndpoint,
    req: SendRequest,
    cancel: CancellationToken,
    mut progress: ProgressReporter,
) -> Result<TransferSummary, TransferError> {
    progress.set_state(TransferState::Connecting);

    let conn = tokio::select! {
        _ = cancel.cancelled() => return finish(&mut progress, Err(TransferError::Cancelled)),
        r = connect(ep, &req) => match r {
            Ok(c) => c,
            Err(e) => return finish(&mut progress, Err(e)),
        },
    };

    let result = tokio::select! {
        _ = cancel.cancelled() => {
            conn.close(VarInt::from_u32(CLOSE_CANCELLED), b"cancelled");
            Err(TransferError::Cancelled)
        }
        r = run(&conn, &req, &mut progress) => r.map_err(|e| e.refine(&conn)),
    };
    match &result {
        Ok(_) => conn.close(VarInt::from_u32(CLOSE_DONE), b"done"),
        Err(TransferError::Protocol(_)) => {
            conn.close(VarInt::from_u32(CLOSE_PROTOCOL), b"protocol error")
        }
        Err(TransferError::Cancelled) => {}
        Err(_) => conn.close(VarInt::from_u32(CLOSE_DONE), b"error"),
    }
    finish(&mut progress, result)
}

fn finish(
    progress: &mut ProgressReporter,
    r: Result<TransferSummary, TransferError>,
) -> Result<TransferSummary, TransferError> {
    progress.set_state(match &r {
        Ok(_) => TransferState::Completed,
        Err(TransferError::Rejected(_)) => TransferState::Rejected,
        Err(e) if e.is_cancel() => TransferState::Cancelled,
        Err(_) => TransferState::Failed,
    });
    r
}

async fn connect(ep: &FlashEndpoint, req: &SendRequest) -> Result<Connection, TransferError> {
    let connecting = ep.endpoint.connect(req.addr, "bolt.local")?;
    let conn = timeout(req.config.connect_timeout, connecting)
        .await
        .map_err(|_| TransferError::Timeout("connecting"))??;
    if let Some(expected) = &req.expected_peer {
        if peer_id(&conn).as_ref() != Some(expected) {
            conn.close(VarInt::from_u32(CLOSE_PROTOCOL), b"identity mismatch");
            return Err(TransferError::PeerMismatch);
        }
    }
    Ok(conn)
}

/// Exchange HELLOs on a freshly opened stream (initiator side).
pub(crate) async fn handshake_initiator(
    conn: &Connection,
    local: &LocalInfo,
    cfg: &TransferConfig,
) -> Result<(SendStream, RecvStream, PeerInfo), TransferError> {
    let id = peer_id(conn)
        .ok_or_else(|| TransferError::Protocol("peer presented no certificate".into()))?;
    let (mut tx, mut rx) = conn.open_bi().await?;
    write_message(&mut tx, &hello(local, cfg)).await?;
    let Message::Hello(h) = timeout(cfg.handshake_timeout, read_message(&mut rx))
        .await
        .map_err(|_| TransferError::Timeout("peer hello"))??
    else {
        return Err(TransferError::Protocol("expected HELLO".into()));
    };
    h.check_version()?;
    Ok((
        tx,
        rx,
        PeerInfo {
            id,
            name: h.device_name,
            kind: h.device_kind,
        },
    ))
}

async fn run(
    conn: &Connection,
    req: &SendRequest,
    progress: &mut ProgressReporter,
) -> Result<TransferSummary, TransferError> {
    let (mut tx, mut rx, peer) = handshake_initiator(conn, &req.local, &req.config).await?;

    let offer = TransferOffer {
        transfer_id: new_transfer_id(),
        files: req
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| FileOffer {
                index: i as u32,
                path: it.rel_path.clone(),
                size: it.size,
                file_id: it.file_id.clone(),
                kind: if it.inline.is_some() {
                    ItemKind::Text
                } else {
                    ItemKind::File
                },
            })
            .collect(),
    };
    offer.validate()?;
    let total = offer.total_bytes();
    progress.init(&offer.transfer_id, total, offer.files.len() as u32);
    write_message(&mut tx, &Message::TransferOffer(offer.clone())).await?;

    progress.set_state(TransferState::AwaitingApproval);
    let reply = timeout(req.config.approval_timeout, read_message(&mut rx))
        .await
        .map_err(|_| TransferError::Timeout("waiting for the receiver to accept"))??;
    let accept = match reply {
        Message::TransferAccept(a) if a.transfer_id == offer.transfer_id => a,
        Message::TransferReject(r) => return Err(TransferError::Rejected(r.reason)),
        Message::TransferError(e) => return Err(TransferError::Remote(e.message)),
        _ => {
            return Err(TransferError::Protocol(
                "expected TRANSFER_ACCEPT or TRANSFER_REJECT".into(),
            ))
        }
    };

    let resume = accept.resume;
    let find = |i: u32| resume.iter().find(|r| r.index == i).cloned();
    // Validate receiver-supplied resume data before trusting it.
    for r in &resume {
        let f = offer
            .files
            .get(r.index as usize)
            .ok_or_else(|| TransferError::Protocol("resume index out of range".into()))?;
        if r.offset > f.size || (r.complete && r.offset != f.size) {
            return Err(TransferError::Protocol("resume offset out of range".into()));
        }
    }

    let already: u64 = resume.iter().map(|r| r.offset).sum();
    progress.set_bytes(already);
    progress.set_state(TransferState::Transferring);
    // Throughput is measured from here, excluding connect time and the wait for the user to accept.
    let started = Instant::now();

    let mut buf = vec![0u8; req.config.clamped_chunk()];
    let mut st = StageTimes::default();
    for (item, fo) in req.items.iter().zip(&offer.files) {
        progress.begin_file(&item.rel_path);
        let rp = find(fo.index);
        if rp.as_ref().is_some_and(|r| r.complete) {
            progress.file_done();
            continue;
        }
        let offset = rp.map_or(0, |r| r.offset);
        if let Some(bytes) = &item.inline {
            send_inline(&mut tx, &mut rx, fo, bytes, progress, &mut st).await?;
            progress.file_done();
            continue;
        }
        send_one(
            &mut tx, &mut rx, item, fo, offset, &mut buf, progress, &mut st,
        )
        .await?;
        progress.file_done();
    }

    write_message(&mut tx, &Message::TransferComplete).await?;
    let t = Instant::now();
    let done = read_message(&mut rx).await?;
    st.finalize += t.elapsed();
    match done {
        Message::TransferComplete => {}
        Message::TransferError(e) => return Err(TransferError::Remote(e.message)),
        _ => return Err(TransferError::Protocol("expected TRANSFER_COMPLETE".into())),
    }
    let _ = tx.finish();
    let path = PathInfo::of(conn);
    tracing::info!(target: "bolt::timing", role = "send", bytes = total, "{st} | {path}");
    Ok(TransferSummary {
        transfer_id: offer.transfer_id,
        peer,
        files: offer.files.len() as u32,
        bytes: total,
        elapsed: started.elapsed(),
        timings: st,
        path,
    })
}

/// Send an in-memory text item through the same FILE_START / FILE_COMPLETE / FILE_ACK exchange
/// as a file, so it gets the same consent step and BLAKE3 verification.
async fn send_inline(
    tx: &mut SendStream,
    rx: &mut RecvStream,
    fo: &FileOffer,
    bytes: &[u8],
    progress: &mut ProgressReporter,
    st: &mut StageTimes,
) -> Result<(), TransferError> {
    write_message(
        tx,
        &Message::FileStart(FileStart {
            index: fo.index,
            offset: 0,
            length: bytes.len() as u64,
        }),
    )
    .await?;
    let t = Instant::now();
    tx.write_all(bytes).await.map_err(TransferError::from)?;
    st.net += t.elapsed();
    st.bytes += bytes.len() as u64;
    progress.add_bytes(bytes.len() as u64);
    write_message(
        tx,
        &Message::FileComplete(FileComplete {
            index: fo.index,
            blake3: blake3::hash(bytes).to_hex().to_string(),
        }),
    )
    .await?;
    match read_message(rx).await? {
        Message::FileAck(a) if a.index == fo.index && a.ok => Ok(()),
        Message::FileAck(a) if a.index == fo.index => Err(TransferError::Integrity(
            a.error.unwrap_or_else(|| "text".into()),
        )),
        Message::TransferError(e) => Err(TransferError::Remote(e.message)),
        _ => Err(TransferError::Protocol("expected FILE_ACK".into())),
    }
}

#[allow(clippy::too_many_arguments)]
async fn send_one(
    tx: &mut SendStream,
    rx: &mut RecvStream,
    item: &SendItem,
    fo: &FileOffer,
    offset: u64,
    buf: &mut [u8],
    progress: &mut ProgressReporter,
    st: &mut StageTimes,
) -> Result<(), TransferError> {
    let blocking_item = item.clone();
    let std_file = tokio::task::spawn_blocking(move || blocking_item.open_blocking())
        .await
        .map_err(|e| TransferError::Io(std::io::Error::other(e)))??;
    let mut file = File::from_std(std_file);
    if file.metadata().await?.len() != fo.size {
        return Err(TransferError::Protocol(format!(
            "{} changed while preparing to send",
            item.rel_path
        )));
    }

    // The whole-file hash covers the resumed prefix too, so hash it locally first.
    let mut hasher = blake3::Hasher::new();
    let mut left = offset;
    while left > 0 {
        let want = buf.len().min(left as usize);
        let t = Instant::now();
        file.read_exact(&mut buf[..want]).await?;
        st.disk += t.elapsed();
        let t = Instant::now();
        hasher.update(&buf[..want]);
        st.hash += t.elapsed();
        left -= want as u64;
    }
    file.seek(std::io::SeekFrom::Start(offset)).await?;

    let length = fo.size - offset;
    write_message(
        tx,
        &Message::FileStart(FileStart {
            index: fo.index,
            offset,
            length,
        }),
    )
    .await?;

    let mut remaining = length;
    while remaining > 0 {
        let want = buf.len().min(remaining as usize);
        let t = Instant::now();
        let n = file.read(&mut buf[..want]).await?;
        st.disk += t.elapsed();
        if n == 0 {
            let _ = write_message(
                tx,
                &Message::TransferError(bolt_protocol::TransferError {
                    code: ErrorCode::Io,
                    message: format!("{} was truncated while sending", item.rel_path),
                }),
            )
            .await;
            return Err(TransferError::Protocol(format!(
                "{} was truncated while sending",
                item.rel_path
            )));
        }
        let t = Instant::now();
        hasher.update(&buf[..n]);
        st.hash += t.elapsed();
        // write_all awaits QUIC flow control: this is the backpressure point.
        let t = Instant::now();
        tx.write_all(&buf[..n]).await.map_err(TransferError::from)?;
        st.net += t.elapsed();
        st.net_calls += 1;
        st.bytes += n as u64;
        remaining -= n as u64;
        progress.add_bytes(n as u64);
    }

    write_message(
        tx,
        &Message::FileComplete(FileComplete {
            index: fo.index,
            blake3: hasher.finalize().to_hex().to_string(),
        }),
    )
    .await?;
    let t = Instant::now();
    let reply = read_message(rx).await?;
    st.finalize += t.elapsed();
    match reply {
        Message::FileAck(a) if a.index == fo.index && a.ok => Ok(()),
        Message::FileAck(a) if a.index == fo.index => Err(TransferError::Integrity(
            a.error.unwrap_or_else(|| item.rel_path.clone()),
        )),
        Message::TransferError(e) => Err(match e.code {
            ErrorCode::Integrity => TransferError::Integrity(e.message),
            _ => TransferError::Remote(e.message),
        }),
        _ => Err(TransferError::Protocol("expected FILE_ACK".into())),
    }
}

/// Connect to `addr`, exchange HELLOs and hang up: learns who is listening there
/// (identity, name, kind) without offering anything. Used to find a device when mDNS
/// can't reach it (for example the phone that hosts the hotspot this computer joined).
pub async fn probe_peer(
    ep: &FlashEndpoint,
    addr: SocketAddr,
    local: &LocalInfo,
    config: &TransferConfig,
) -> Result<PeerInfo, TransferError> {
    let connecting = ep.endpoint.connect(addr, "bolt.local")?;
    let conn = timeout(config.connect_timeout, connecting)
        .await
        .map_err(|_| TransferError::Timeout("connecting"))??;
    let result = handshake_initiator(&conn, local, config)
        .await
        .map(|(_, _, peer)| peer);
    conn.close(VarInt::from_u32(CLOSE_DONE), b"probe");
    result
}
