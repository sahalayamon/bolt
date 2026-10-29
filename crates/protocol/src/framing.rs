use crate::message::Message;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Largest accepted control frame. Bounds allocation from untrusted peers.
pub const MAX_FRAME_BYTES: u32 = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame of {0} bytes exceeds the {MAX_FRAME_BYTES} byte limit")]
    TooLarge(u32),
    #[error("malformed message: {0}")]
    Malformed(#[from] serde_json::Error),
}

pub async fn write_message<W: AsyncWrite + Unpin>(
    w: &mut W,
    msg: &Message,
) -> Result<(), FrameError> {
    let body = serde_json::to_vec(msg)?;
    let len = u32::try_from(body.len()).map_err(|_| FrameError::TooLarge(u32::MAX))?;
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len));
    }
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&body);
    w.write_all(&frame).await?;
    Ok(())
}

pub async fn read_message<R: AsyncRead + Unpin>(r: &mut R) -> Result<Message, FrameError> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body).await?;
    Ok(serde_json::from_slice(&body)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::*;

    #[tokio::test]
    async fn roundtrip() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let msg = Message::TransferReject(TransferReject {
            reason: "no".into(),
        });
        write_message(&mut a, &msg).await.unwrap();
        assert_eq!(read_message(&mut b).await.unwrap(), msg);
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected_before_allocation() {
        let (mut a, mut b) = tokio::io::duplex(64);
        a.write_all(&(MAX_FRAME_BYTES + 1).to_be_bytes())
            .await
            .unwrap();
        assert!(matches!(
            read_message(&mut b).await,
            Err(FrameError::TooLarge(_))
        ));
    }

    #[tokio::test]
    async fn garbage_is_malformed_not_panic() {
        let (mut a, mut b) = tokio::io::duplex(64);
        a.write_all(&5u32.to_be_bytes()).await.unwrap();
        a.write_all(b"nope!").await.unwrap();
        assert!(matches!(
            read_message(&mut b).await,
            Err(FrameError::Malformed(_))
        ));
    }
}
