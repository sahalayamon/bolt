//! Where does the time go? Per-stage accounting for a transfer, plus the QUIC path's own statistics.
//!
//! Stages are measured by wall-clock around the *await points* of the hot loops, so "net" is time the
//! task spent waiting for the network (read side) or for flow control (write side), "disk" is time in
//! file I/O, "hash" is BLAKE3 on the task thread, and "sync"/"finalize" are fsync and the atomic rename.
//! They are sequential in the loops, so they sum to roughly the transfer time.

use std::{fmt, time::Duration};

#[derive(Debug, Clone, Copy, Default)]
pub struct StageTimes {
    /// Waiting on the network: `read` (receiver) / `write_all` through flow control (sender).
    pub net: Duration,
    /// File reads (sender) or writes (receiver).
    pub disk: Duration,
    /// BLAKE3 hashing.
    pub hash: Duration,
    /// `sync_data` after the last byte of a file (receiver).
    pub sync: Duration,
    /// Atomic rename / publish and ack round trips (receiver) or waiting for the final ack (sender).
    pub finalize: Duration,
    /// Number of network read/write calls and the bytes they moved (mean call size = bytes / calls).
    pub net_calls: u64,
    pub bytes: u64,
}

impl StageTimes {
    pub fn total(&self) -> Duration {
        self.net + self.disk + self.hash + self.sync + self.finalize
    }

    pub fn mean_call_bytes(&self) -> u64 {
        self.bytes.checked_div(self.net_calls).unwrap_or(0)
    }
}

impl fmt::Display for StageTimes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total = self.total().as_secs_f64().max(1e-9);
        let part = |d: Duration| {
            format!(
                "{:.2}s ({:.0}%)",
                d.as_secs_f64(),
                d.as_secs_f64() * 100.0 / total
            )
        };
        write!(
            f,
            "net {} · disk {} · hash {} · sync {} · finalize {} · {} calls × ~{} B",
            part(self.net),
            part(self.disk),
            part(self.hash),
            part(self.sync),
            part(self.finalize),
            self.net_calls,
            self.mean_call_bytes()
        )
    }
}

/// Snapshot of what QUIC itself observed on the connection (loss, RTT, window, datagram size).
#[derive(Debug, Clone, Copy, Default)]
pub struct PathInfo {
    pub rtt_ms: f64,
    pub cwnd_bytes: u64,
    pub lost_packets: u64,
    pub sent_packets: u64,
    pub congestion_events: u64,
    pub mtu: u16,
}

impl PathInfo {
    pub fn of(conn: &quinn::Connection) -> Self {
        let s = conn.stats();
        Self {
            rtt_ms: s.path.rtt.as_secs_f64() * 1000.0,
            cwnd_bytes: s.path.cwnd,
            lost_packets: s.path.lost_packets,
            sent_packets: s.path.sent_packets,
            congestion_events: s.path.congestion_events,
            mtu: s.path.current_mtu,
        }
    }

    pub fn loss_pct(&self) -> f64 {
        if self.sent_packets == 0 {
            0.0
        } else {
            self.lost_packets as f64 * 100.0 / self.sent_packets as f64
        }
    }
}

impl fmt::Display for PathInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rtt {:.1} ms · cwnd {} KiB · mtu {} · lost {}/{} pkts ({:.2}%) · {} congestion events",
            self.rtt_ms,
            self.cwnd_bytes / 1024,
            self.mtu,
            self.lost_packets,
            self.sent_packets,
            self.loss_pct(),
            self.congestion_events
        )
    }
}
