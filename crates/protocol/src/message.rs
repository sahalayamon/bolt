use crate::{path, MAX_FILES_PER_OFFER, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Message {
    Hello(Hello),
    PairRequest(PairRequest),
    PairResponse(PairResponse),
    TransferOffer(TransferOffer),
    TransferAccept(TransferAccept),
    TransferReject(TransferReject),
    FileStart(FileStart),
    FileComplete(FileComplete),
    FileAck(FileAck),
    TransferComplete,
    TransferError(TransferError),
    /// Network speed test: no files, no disk, no hashing. See [`LinkTest`].
    LinkTest(LinkTest),
    LinkTestResult(LinkTestResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkDirection {
    /// The requester sends `bytes` raw bytes; the responder discards them and answers with [`LinkTestResult`].
    Up,
    /// The responder sends `bytes` raw bytes and finishes its stream; the requester discards them.
    Down,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkTest {
    pub bytes: u64,
    pub direction: LinkDirection,
}

/// Sent by the receiving side of an `Up` test once all bytes arrived (its own clock, so no round trip is counted).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkTestResult {
    pub bytes: u64,
    pub elapsed_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u16,
    /// Human-readable name. Untrusted display text; never an identity.
    pub device_name: String,
    pub device_kind: DeviceKind,
    pub max_chunk_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Desktop,
    Phone,
    Tablet,
    Unknown,
}

/// `proof` = keyed BLAKE3 over both certificate fingerprints, keyed from the
/// short-lived pairing token. The token itself never crosses the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairRequest {
    pub proof: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairResponse {
    pub approved: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileOffer {
    pub index: u32,
    /// Relative, `/`-separated, validated by [`path::validate_rel_path`].
    pub path: String,
    pub size: u64,
    /// Opaque sender-chosen identity of the source file version (e.g. size+mtime),
    /// used to decide whether a partial file may be resumed.
    pub file_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferOffer {
    pub transfer_id: String,
    pub files: Vec<FileOffer>,
}

impl TransferOffer {
    pub fn total_bytes(&self) -> u64 {
        self.files
            .iter()
            .fold(0u64, |a, f| a.saturating_add(f.size))
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.transfer_id.len() != 32 || !self.transfer_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(ValidationError::BadTransferId);
        }
        if self.files.is_empty() {
            return Err(ValidationError::EmptyOffer);
        }
        if self.files.len() > MAX_FILES_PER_OFFER {
            return Err(ValidationError::TooManyFiles(self.files.len()));
        }
        let mut seen = std::collections::HashSet::new();
        for (i, f) in self.files.iter().enumerate() {
            if f.index as usize != i {
                return Err(ValidationError::BadIndex(f.index));
            }
            path::validate_rel_path(&f.path)
                .map_err(|e| ValidationError::BadPath(f.path.clone(), e))?;
            if !seen.insert(f.path.to_lowercase()) {
                return Err(ValidationError::DuplicatePath(f.path.clone()));
            }
            if f.file_id.len() > 128 {
                return Err(ValidationError::BadFileId);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumePoint {
    pub index: u32,
    /// Bytes the receiver already holds (and will re-verify by hashing).
    pub offset: u64,
    /// The file was fully received and finalised earlier; send nothing for it.
    #[serde(default)]
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferAccept {
    pub transfer_id: String,
    pub resume: Vec<ResumePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferReject {
    pub reason: String,
}

/// Followed by exactly `length` raw bytes of file content starting at `offset`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileStart {
    pub index: u32,
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileComplete {
    pub index: u32,
    /// Hex BLAKE3 of the entire file (including any resumed prefix).
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileAck {
    pub index: u32,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Protocol,
    Io,
    DiskFull,
    Integrity,
    Internal,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("transfer id must be 32 hex characters")]
    BadTransferId,
    #[error("offer contains no files")]
    EmptyOffer,
    #[error("offer contains {0} files, above the limit")]
    TooManyFiles(usize),
    #[error("file index {0} out of sequence")]
    BadIndex(u32),
    #[error("unsafe path {0:?}: {1}")]
    BadPath(String, path::PathError),
    #[error("duplicate path {0:?}")]
    DuplicatePath(String),
    #[error("file id too long")]
    BadFileId,
    #[error("protocol version {0} is not supported (this build speaks {PROTOCOL_VERSION})")]
    UnsupportedVersion(u16),
}

impl Hello {
    pub fn check_version(&self) -> Result<(), ValidationError> {
        if self.protocol == PROTOCOL_VERSION {
            Ok(())
        } else {
            Err(ValidationError::UnsupportedVersion(self.protocol))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(paths: &[&str]) -> TransferOffer {
        TransferOffer {
            transfer_id: "0123456789abcdef0123456789abcdef".into(),
            files: paths
                .iter()
                .enumerate()
                .map(|(i, p)| FileOffer {
                    index: i as u32,
                    path: (*p).into(),
                    size: 1,
                    file_id: "x".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn accepts_normal_offer() {
        offer(&["a.txt", "dir/b.txt", "дом/файл.txt"])
            .validate()
            .unwrap();
    }

    #[test]
    fn rejects_traversal_and_duplicates() {
        assert!(offer(&["../evil"]).validate().is_err());
        assert!(offer(&["a/../../evil"]).validate().is_err());
        assert!(offer(&["/abs"]).validate().is_err());
        assert!(offer(&["A.txt", "a.TXT"]).validate().is_err());
        assert!(offer(&[]).validate().is_err());
    }

    #[test]
    fn rejects_bad_index_and_id() {
        let mut o = offer(&["a", "b"]);
        o.files[1].index = 5;
        assert!(o.validate().is_err());
        let mut o = offer(&["a"]);
        o.transfer_id = "short".into();
        assert!(o.validate().is_err());
    }

    #[test]
    fn version_mismatch() {
        let h = Hello {
            protocol: 99,
            device_name: "x".into(),
            device_kind: DeviceKind::Desktop,
            max_chunk_bytes: 1,
        };
        assert!(h.check_version().is_err());
    }

    #[test]
    fn json_shape_is_stable() {
        let m = Message::TransferComplete;
        assert_eq!(
            serde_json::to_string(&m).unwrap(),
            r#"{"type":"TRANSFER_COMPLETE"}"#
        );
    }
}
