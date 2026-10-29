use crate::{
    endpoint::{peer_id, FlashEndpoint},
    error::TransferError,
    receive::now_unix,
    send::{handshake_initiator, LocalInfo, PeerInfo},
    TransferConfig, CLOSE_DONE, CLOSE_PROTOCOL,
};
use bolt_protocol::{read_message, write_message, Message, PairRequest};
use bolt_security::{
    pairing::make_proof, verification_code, PairingUri, TrustStore, TrustedDevice,
};
use quinn::VarInt;
use std::{net::SocketAddr, time::Duration};
use tokio::time::timeout;

const APPROVAL_TIMEOUT: Duration = Duration::from_secs(130);

/// Initiator side of pairing (the device that scanned the QR code).
///
/// `on_code` is called with the six-digit verification code as soon as it is
/// known, so the UI can display it while the other user approves.
/// On success the peer is added to `trust`.
pub async fn pair_with(
    ep: &FlashEndpoint,
    addr: SocketAddr,
    uri: &PairingUri,
    local: &LocalInfo,
    config: &TransferConfig,
    trust: &dyn TrustStore,
    on_code: impl FnOnce(String),
) -> Result<PeerInfo, TransferError> {
    let conn = timeout(
        config.connect_timeout,
        ep.endpoint.connect(addr, "bolt.local")?,
    )
    .await
    .map_err(|_| TransferError::Timeout("connecting"))??;

    // The QR pinned the responder's identity; anything else is a possible impostor.
    let remote = peer_id(&conn)
        .ok_or_else(|| TransferError::Protocol("peer presented no certificate".into()))?;
    if remote != uri.device_id {
        conn.close(VarInt::from_u32(CLOSE_PROTOCOL), b"identity mismatch");
        return Err(TransferError::PeerMismatch);
    }

    let result = async {
        let (mut tx, mut rx, peer) = handshake_initiator(&conn, local, config).await?;
        on_code(verification_code(ep.device_id(), &remote, &uri.token));
        let proof = make_proof(&uri.token, ep.device_id(), &remote);
        write_message(&mut tx, &Message::PairRequest(PairRequest { proof })).await?;
        match timeout(APPROVAL_TIMEOUT, read_message(&mut rx))
            .await
            .map_err(|_| {
                TransferError::Timeout("waiting for the other device to approve pairing")
            })?? {
            Message::PairResponse(r) if r.approved => Ok(peer),
            Message::PairResponse(r) => Err(TransferError::Rejected(
                r.reason
                    .unwrap_or_else(|| "pairing was not approved".into()),
            )),
            _ => Err(TransferError::Protocol("expected PAIR_RESPONSE".into())),
        }
    }
    .await
    .map_err(|e| e.refine(&conn));

    conn.close(VarInt::from_u32(CLOSE_DONE), b"done");
    let peer = result?;
    trust
        .trust(TrustedDevice {
            id: peer.id.clone(),
            name: peer.name.clone(),
            paired_at_unix: now_unix(),
            last_seen_unix: Some(now_unix()),
        })
        .map_err(|e| TransferError::Setup(e.to_string()))?;
    Ok(peer)
}
