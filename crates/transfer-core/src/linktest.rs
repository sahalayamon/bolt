//! Pure network speed test over the real QUIC path: no files, no disk, no hashing.
//!
//! Comparing this number with a real transfer's speed separates "the link is slow" from
//! "disk/CPU/our code is slow". Both directions are measured, each on its own connection.

use crate::{
    endpoint::{peer_id, FlashEndpoint},
    error::TransferError,
    send::{handshake_initiator, LocalInfo},
    stats::PathInfo,
    TransferConfig, CLOSE_DONE,
};
use bolt_protocol::{
    read_message, write_message, LinkDirection, LinkTest, LinkTestResult, Message,
    LINK_TEST_MAX_BYTES,
};
use bolt_security::DeviceId;
use quinn::{Connection, RecvStream, SendStream, VarInt};
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{sync::Semaphore, time::timeout};
use tokio_util::sync::CancellationToken;

const BLOCK: usize = 256 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct DirectionReport {
    pub bytes: u64,
    pub elapsed: Duration,
    pub path: PathInfo,
}

impl DirectionReport {
    /// Megabytes (10^6 bytes) per second.
    pub fn mb_per_s(&self) -> f64 {
        self.bytes as f64 / 1e6 / self.elapsed.as_secs_f64().max(1e-9)
    }

    pub fn mbit_per_s(&self) -> f64 {
        self.mb_per_s() * 8.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LinkReport {
    /// This device → peer.
    pub up: DirectionReport,
    /// Peer → this device.
    pub down: DirectionReport,
    /// Time to establish the QUIC connection and exchange HELLOs (≈ a few round trips).
    pub connect: Duration,
}

/// A deterministic non-trivial pattern (so nothing on the path can compress it away).
fn pattern() -> Vec<u8> {
    (0..BLOCK as u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8)
        .collect()
}

/// Run the speed test against `addr`, moving `bytes` in each direction.
pub async fn link_test(
    ep: &FlashEndpoint,
    addr: SocketAddr,
    expected_peer: Option<&DeviceId>,
    local: &LocalInfo,
    bytes: u64,
    config: &TransferConfig,
    cancel: CancellationToken,
) -> Result<LinkReport, TransferError> {
    let bytes = bytes.clamp(1, LINK_TEST_MAX_BYTES);
    tokio::select! {
        _ = cancel.cancelled() => Err(TransferError::Cancelled),
        r = run(ep, addr, expected_peer, local, bytes, config) => r,
    }
}

async fn open(
    ep: &FlashEndpoint,
    addr: SocketAddr,
    expected: Option<&DeviceId>,
    local: &LocalInfo,
    config: &TransferConfig,
) -> Result<(Connection, SendStream, RecvStream, Duration), TransferError> {
    let t = Instant::now();
    let connecting = ep.endpoint.connect(addr, "bolt.local")?;
    let conn = timeout(config.connect_timeout, connecting)
        .await
        .map_err(|_| TransferError::Timeout("connecting"))??;
    if let Some(e) = expected {
        if peer_id(&conn).as_ref() != Some(e) {
            conn.close(VarInt::from_u32(2), b"identity mismatch");
            return Err(TransferError::PeerMismatch);
        }
    }
    let (tx, rx, _peer) = handshake_initiator(&conn, local, config).await?;
    Ok((conn, tx, rx, t.elapsed()))
}

async fn run(
    ep: &FlashEndpoint,
    addr: SocketAddr,
    expected: Option<&DeviceId>,
    local: &LocalInfo,
    bytes: u64,
    config: &TransferConfig,
) -> Result<LinkReport, TransferError> {
    let block = pattern();

    // ---- up: we send, the peer discards and reports its own receive time ----
    let (conn, mut tx, mut rx, connect) = open(ep, addr, expected, local, config).await?;
    write_message(
        &mut tx,
        &Message::LinkTest(LinkTest {
            bytes,
            direction: LinkDirection::Up,
        }),
    )
    .await?;
    let t = Instant::now();
    let mut left = bytes;
    while left > 0 {
        let n = (block.len() as u64).min(left) as usize;
        tx.write_all(&block[..n])
            .await
            .map_err(TransferError::from)?;
        left -= n as u64;
    }
    let reply = timeout(Duration::from_secs(60), read_message(&mut rx))
        .await
        .map_err(|_| TransferError::Timeout("link test result"))??;
    let wall = t.elapsed();
    let Message::LinkTestResult(res) = reply else {
        return Err(TransferError::Protocol("expected LINK_TEST_RESULT".into()));
    };
    // The peer's clock excludes the final ack round trip; ours includes the send-side buffering.
    // Use the larger of the two so the figure is never flattering.
    let up = DirectionReport {
        bytes,
        elapsed: wall.max(Duration::from_micros(res.elapsed_micros)),
        path: PathInfo::of(&conn),
    };
    let _ = tx.finish();
    conn.close(VarInt::from_u32(CLOSE_DONE), b"link test done");

    // ---- down: the peer sends, we discard ----
    let (conn, mut tx, mut rx, _) = open(ep, addr, expected, local, config).await?;
    write_message(
        &mut tx,
        &Message::LinkTest(LinkTest {
            bytes,
            direction: LinkDirection::Down,
        }),
    )
    .await?;
    let t = Instant::now();
    let mut buf = vec![0u8; BLOCK];
    let mut got = 0u64;
    while got < bytes {
        match rx.read(&mut buf).await.map_err(TransferError::from)? {
            Some(n) => got += n as u64,
            None => break,
        }
    }
    let down = DirectionReport {
        bytes: got,
        elapsed: t.elapsed(),
        path: PathInfo::of(&conn),
    };
    let _ = tx.finish();
    conn.close(VarInt::from_u32(CLOSE_DONE), b"link test done");
    if got < bytes {
        return Err(TransferError::Protocol(
            "peer ended the download early".into(),
        ));
    }
    Ok(LinkReport { up, down, connect })
}

/// Responder side (called by the receiver after the HELLO exchange).
pub(crate) async fn serve(
    slots: &Semaphore,
    conn: &Connection,
    mut tx: SendStream,
    mut rx: RecvStream,
    req: LinkTest,
) -> Result<(), TransferError> {
    if req.bytes == 0 || req.bytes > LINK_TEST_MAX_BYTES {
        return Err(TransferError::Protocol(
            "link test size out of range".into(),
        ));
    }
    // A speed test counts as a transfer: it must not starve real ones or run in parallel with many others.
    let Ok(_slot) = slots.try_acquire() else {
        return Err(TransferError::Rejected("busy".into()));
    };
    match req.direction {
        LinkDirection::Up => {
            let t = Instant::now();
            let mut buf = vec![0u8; BLOCK];
            let mut got = 0u64;
            while got < req.bytes {
                match rx.read(&mut buf).await.map_err(TransferError::from)? {
                    Some(n) => got += n as u64,
                    None => break,
                }
            }
            write_message(
                &mut tx,
                &Message::LinkTestResult(LinkTestResult {
                    bytes: got,
                    elapsed_micros: t.elapsed().as_micros() as u64,
                }),
            )
            .await?;
            let _ = tx.finish();
        }
        LinkDirection::Down => {
            let block = pattern();
            let mut left = req.bytes;
            while left > 0 {
                let n = (block.len() as u64).min(left) as usize;
                tx.write_all(&block[..n])
                    .await
                    .map_err(TransferError::from)?;
                left -= n as u64;
            }
            let _ = tx.finish();
            // Do not let the caller close the connection until the peer has actually received everything:
            // on a slow link the data is still in flight long after `write_all` returned.
            let _ = timeout(Duration::from_secs(180), tx.stopped()).await;
        }
    }
    let _ = timeout(Duration::from_secs(5), conn.closed()).await;
    Ok(())
}
