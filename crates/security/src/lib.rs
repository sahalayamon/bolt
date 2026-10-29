//! Device identity, TLS configuration, pairing and trust.
//!
//! Identity model: every install owns a self-signed ECDSA P-256 certificate.
//! A peer is identified by the BLAKE3 hash of that certificate (its
//! [`DeviceId`]). TLS only proves the peer holds the key for that certificate;
//! whether the identity is *trusted* is decided by pairing (see [`pairing`]).

pub mod identity;
pub mod pairing;
pub mod tls;
pub mod trust;

pub use identity::{DeviceId, Identity, IdentityError};
pub use pairing::{verification_code, PairError, PairingSession, PairingUri};
pub use trust::{MemoryTrustStore, TrustError, TrustStore, TrustedDevice};
