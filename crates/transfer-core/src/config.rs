use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TransferConfig {
    /// Read/write buffer per stream. Memory use is O(chunk_size) per active transfer.
    pub chunk_size: usize,
    pub connect_timeout: Duration,
    /// Max time to wait for the first protocol message from a new connection.
    pub handshake_timeout: Duration,
    /// Max time a sender waits for the receiving user to accept.
    pub approval_timeout: Duration,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            chunk_size: 256 * 1024,
            connect_timeout: Duration::from_secs(10),
            handshake_timeout: Duration::from_secs(10),
            approval_timeout: Duration::from_secs(120),
        }
    }
}

impl TransferConfig {
    pub const MIN_CHUNK: usize = 16 * 1024;
    pub const MAX_CHUNK: usize = 4 * 1024 * 1024;

    pub fn clamped_chunk(&self) -> usize {
        self.chunk_size.clamp(Self::MIN_CHUNK, Self::MAX_CHUNK)
    }
}
