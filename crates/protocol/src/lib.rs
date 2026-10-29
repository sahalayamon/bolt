//! Bolt wire protocol, version 1.
//!
//! Control messages are length-prefixed JSON frames on a QUIC bidirectional
//! stream. File bytes follow a `FILE_START` frame as raw bytes (no framing),
//! so the data path has zero per-chunk overhead. See `docs/protocol.md`.

pub mod framing;
pub mod message;
pub mod path;

pub use framing::{read_message, write_message, FrameError, MAX_FRAME_BYTES};
pub use message::*;

/// Protocol major version carried in `HELLO`. Peers with different majors refuse to talk.
pub const PROTOCOL_VERSION: u16 = 1;
/// ALPN identifier negotiated during the TLS 1.3 / QUIC handshake.
pub const ALPN: &[u8] = b"bolt/1";
/// mDNS / DNS-SD service type.
pub const SERVICE_TYPE: &str = "_bolt._udp.local.";
/// Upper bound on files in a single offer (bounds receiver memory).
pub const MAX_FILES_PER_OFFER: usize = 50_000;
/// Largest network speed test a peer may request (it burns the other side's bandwidth).
pub const LINK_TEST_MAX_BYTES: u64 = 1 << 30;
