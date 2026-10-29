//! Short-lived pairing sessions, proofs and verification codes.
//!
//! Flow: the responder shows a QR code ([`PairingUri`]) containing its device
//! id (pin), address and a random single-use token valid for a few minutes.
//! The scanner connects, verifies the TLS identity equals the pinned id, and
//! sends `proof = keyed_hash(token, scanner_id || responder_id)`. A MITM
//! cannot produce the proof without the token, which only travels over the QR
//! (out-of-band). Both sides also derive the same 6-digit code to compare by
//! eye, which additionally protects the manual (no-QR) flow.

use crate::identity::DeviceId;
use rand::RngCore;
use std::time::{Duration, Instant};
use thiserror::Error;

pub const TOKEN_BYTES: usize = 16;
const MAX_FAILED_ATTEMPTS: u8 = 3;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PairError {
    #[error("pairing session expired or already used")]
    Expired,
    #[error("pairing proof did not match")]
    BadProof,
    #[error("malformed pairing uri: {0}")]
    BadUri(&'static str),
}

fn proof_for(
    token: &[u8; TOKEN_BYTES],
    initiator: &DeviceId,
    responder: &DeviceId,
) -> blake3::Hash {
    let key = blake3::derive_key("bolt pairing v1 proof key", token);
    let mut h = blake3::Hasher::new_keyed(&key);
    h.update(&initiator.as_bytes32());
    h.update(&responder.as_bytes32());
    h.finalize()
}

/// Initiator side: compute the proof to send in `PAIR_REQUEST`.
pub fn make_proof(token: &[u8; TOKEN_BYTES], initiator: &DeviceId, responder: &DeviceId) -> String {
    proof_for(token, initiator, responder).to_hex().to_string()
}

/// Six digits (`"123 456"`) both users compare. Order-independent.
pub fn verification_code(a: &DeviceId, b: &DeviceId, token: &[u8; TOKEN_BYTES]) -> String {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let mut h = blake3::Hasher::new_derive_key("bolt pairing v1 verification code");
    h.update(&lo.as_bytes32());
    h.update(&hi.as_bytes32());
    h.update(token);
    let bytes = h.finalize();
    let n = u32::from_be_bytes(bytes.as_bytes()[..4].try_into().expect("4 bytes")) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

/// Responder side state for one pairing attempt.
#[derive(Debug)]
pub struct PairingSession {
    token: [u8; TOKEN_BYTES],
    expires: Instant,
    failures: u8,
    consumed: bool,
}

impl PairingSession {
    pub fn new(ttl: Duration) -> Self {
        let mut token = [0u8; TOKEN_BYTES];
        rand::rng().fill_bytes(&mut token);
        Self {
            token,
            expires: Instant::now() + ttl,
            failures: 0,
            consumed: false,
        }
    }

    pub fn token(&self) -> &[u8; TOKEN_BYTES] {
        &self.token
    }

    pub fn is_live(&self) -> bool {
        !self.consumed && self.failures < MAX_FAILED_ATTEMPTS && Instant::now() < self.expires
    }

    /// Check a proof. Success consumes the session (single use).
    pub fn verify(
        &mut self,
        initiator: &DeviceId,
        responder: &DeviceId,
        proof_hex: &str,
    ) -> Result<(), PairError> {
        if !self.is_live() {
            return Err(PairError::Expired);
        }
        let ok = blake3::Hash::from_hex(proof_hex)
            .map(|p| p == proof_for(&self.token, initiator, responder)) // constant-time eq
            .unwrap_or(false);
        if ok {
            self.consumed = true;
            Ok(())
        } else {
            self.failures += 1;
            Err(PairError::BadProof)
        }
    }
}

/// Contents of the pairing QR code. Contains no long-lived secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingUri {
    pub device_id: DeviceId,
    pub addr: String,
    pub token: [u8; TOKEN_BYTES],
    pub expires_unix: u64,
}

impl PairingUri {
    pub fn encode(&self) -> String {
        format!(
            "bolt://pair?v=1&id={}&addr={}&t={}&exp={}",
            self.device_id,
            self.addr,
            hex::encode(self.token),
            self.expires_unix
        )
    }

    pub fn parse(s: &str) -> Result<Self, PairError> {
        let q = s
            .strip_prefix("bolt://pair?")
            .ok_or(PairError::BadUri("scheme"))?;
        let (mut v, mut id, mut addr, mut t, mut exp) = (None, None, None, None, None);
        for kv in q.split('&') {
            let (k, val) = kv.split_once('=').ok_or(PairError::BadUri("pair"))?;
            match k {
                "v" => v = Some(val),
                "id" => id = Some(val),
                "addr" => addr = Some(val),
                "t" => t = Some(val),
                "exp" => exp = Some(val),
                _ => {}
            }
        }
        if v != Some("1") {
            return Err(PairError::BadUri("version"));
        }
        let device_id = DeviceId::parse(id.ok_or(PairError::BadUri("id"))?)
            .map_err(|_| PairError::BadUri("id"))?;
        let mut token = [0u8; TOKEN_BYTES];
        hex::decode_to_slice(t.ok_or(PairError::BadUri("token"))?, &mut token)
            .map_err(|_| PairError::BadUri("token"))?;
        let expires_unix = exp
            .ok_or(PairError::BadUri("exp"))?
            .parse()
            .map_err(|_| PairError::BadUri("exp"))?;
        Ok(Self {
            device_id,
            addr: addr.ok_or(PairError::BadUri("addr"))?.to_owned(),
            token,
            expires_unix,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (DeviceId, DeviceId) {
        (
            DeviceId::parse(&"1".repeat(64)).unwrap(),
            DeviceId::parse(&"2".repeat(64)).unwrap(),
        )
    }

    #[test]
    fn correct_proof_succeeds_once() {
        let (a, b) = ids();
        let mut s = PairingSession::new(Duration::from_secs(60));
        let proof = make_proof(s.token(), &a, &b);
        s.verify(&a, &b, &proof).unwrap();
        assert_eq!(
            s.verify(&a, &b, &proof),
            Err(PairError::Expired),
            "token is single-use"
        );
    }

    #[test]
    fn wrong_proof_and_lockout() {
        let (a, b) = ids();
        let mut s = PairingSession::new(Duration::from_secs(60));
        for _ in 0..3 {
            assert_eq!(s.verify(&a, &b, &"0".repeat(64)), Err(PairError::BadProof));
        }
        let good = make_proof(s.token(), &a, &b);
        assert_eq!(
            s.verify(&a, &b, &good),
            Err(PairError::Expired),
            "locked after repeated failures"
        );
    }

    #[test]
    fn proof_bound_to_identities() {
        let (a, b) = ids();
        let c = DeviceId::parse(&"3".repeat(64)).unwrap();
        let mut s = PairingSession::new(Duration::from_secs(60));
        let proof = make_proof(s.token(), &a, &b);
        assert_eq!(
            s.verify(&c, &b, &proof),
            Err(PairError::BadProof),
            "a MITM identity cannot reuse a proof"
        );
    }

    #[test]
    fn expiry() {
        let (a, b) = ids();
        let mut s = PairingSession::new(Duration::ZERO);
        let proof = make_proof(s.token(), &a, &b);
        assert_eq!(s.verify(&a, &b, &proof), Err(PairError::Expired));
    }

    #[test]
    fn code_is_symmetric_and_token_dependent() {
        let (a, b) = ids();
        let t1 = [1u8; TOKEN_BYTES];
        let t2 = [2u8; TOKEN_BYTES];
        assert_eq!(
            verification_code(&a, &b, &t1),
            verification_code(&b, &a, &t1)
        );
        assert_ne!(
            verification_code(&a, &b, &t1),
            verification_code(&a, &b, &t2)
        );
    }

    #[test]
    fn uri_roundtrip_and_rejects_garbage() {
        let (a, _) = ids();
        let u = PairingUri {
            device_id: a,
            addr: "192.168.1.5:47600".into(),
            token: [7; TOKEN_BYTES],
            expires_unix: 1234,
        };
        assert_eq!(PairingUri::parse(&u.encode()).unwrap(), u);
        assert!(PairingUri::parse("http://x").is_err());
        assert!(PairingUri::parse("bolt://pair?v=2&id=x").is_err());
    }
}
