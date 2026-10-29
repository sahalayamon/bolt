//! Bolt transfer engine: QUIC endpoint, sender, receiver, safe file
//! staging, progress and cancellation.

mod config;
#[cfg(test)]
mod e2e_tests;
mod endpoint;
mod error;
mod fsx;
mod linktest;
mod pairing_flow;
mod progress;
mod receive;
mod scan;
mod send;
#[cfg(test)]
mod sim;
mod stats;

pub use config::TransferConfig;
pub use endpoint::{peer_id, probe, Congestion, EndpointTuning, FlashEndpoint};
pub use error::TransferError;
pub use fsx::Staging;
pub use linktest::{link_test, LinkReport};
pub use pairing_flow::pair_with;
pub use progress::{Progress, ProgressReporter, SpeedMeter, TransferState};
pub use receive::{
    IncomingEvent, IncomingOffer, IncomingPair, OfferDecision, ReceivedSummary, Receiver,
    ReceiverConfig, ReceiverHandle,
};
pub use scan::{finalize_items, scan, FileOpener, SendItem};
pub use send::{probe_peer, send_files, LocalInfo, PeerInfo, SendRequest, TransferSummary};
pub use stats::{PathInfo, StageTimes};

/// QUIC application close code meaning "the user cancelled this transfer".
pub(crate) const CLOSE_CANCELLED: u32 = 1;
/// QUIC application close code for a clean finish.
pub(crate) const CLOSE_DONE: u32 = 0;
/// QUIC application close code for a protocol violation.
pub(crate) const CLOSE_PROTOCOL: u32 = 2;
