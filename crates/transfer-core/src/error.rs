use bolt_protocol::{FrameError, ValidationError};
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransferError {
    #[error("cancelled")]
    Cancelled,
    #[error("cancelled by the other device")]
    PeerCancelled,
    #[error("declined by the receiver: {0}")]
    Rejected(String),
    #[error("the device at this address is not the one expected (identity mismatch)")]
    PeerMismatch,
    #[error("timed out: {0}")]
    Timeout(&'static str),
    #[error("protocol violation: {0}")]
    Protocol(String),
    #[error("the other device reported an error: {0}")]
    Remote(String),
    #[error("integrity check failed for {0}")]
    Integrity(String),
    #[error("not enough free disk space: need {need} bytes, {available} available")]
    DiskFull { need: u64, available: u64 },
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("connection error: {0}")]
    Connection(String),
    #[error("invalid offer: {0}")]
    InvalidOffer(#[from] ValidationError),
    #[error("setup error: {0}")]
    Setup(String),
}

impl From<FrameError> for TransferError {
    fn from(e: FrameError) -> Self {
        match e {
            FrameError::Io(e) => Self::Io(e),
            other => Self::Protocol(other.to_string()),
        }
    }
}

impl From<quinn::ConnectError> for TransferError {
    fn from(e: quinn::ConnectError) -> Self {
        Self::Connection(e.to_string())
    }
}

impl From<quinn::ConnectionError> for TransferError {
    fn from(e: quinn::ConnectionError) -> Self {
        match &e {
            quinn::ConnectionError::ApplicationClosed(c)
                if c.error_code.into_inner() == crate::CLOSE_CANCELLED as u64 =>
            {
                Self::PeerCancelled
            }
            _ => Self::Connection(e.to_string()),
        }
    }
}

impl From<quinn::WriteError> for TransferError {
    fn from(e: quinn::WriteError) -> Self {
        match e {
            quinn::WriteError::ConnectionLost(c) => c.into(),
            other => Self::Connection(other.to_string()),
        }
    }
}

impl From<quinn::ReadError> for TransferError {
    fn from(e: quinn::ReadError) -> Self {
        match e {
            quinn::ReadError::ConnectionLost(c) => c.into(),
            other => Self::Connection(other.to_string()),
        }
    }
}

impl TransferError {
    /// Re-interpret a low-level I/O failure on a stream using the connection's
    /// close reason, so a peer cancel is reported as a cancel rather than "i/o".
    pub(crate) fn refine(self, conn: &quinn::Connection) -> Self {
        match (&self, conn.close_reason()) {
            (Self::Io(_) | Self::Connection(_), Some(reason)) => reason.into(),
            _ => self,
        }
    }

    pub fn is_cancel(&self) -> bool {
        matches!(self, Self::Cancelled | Self::PeerCancelled)
    }
}
