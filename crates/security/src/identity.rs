use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use std::{fmt, fs, io, path::Path};
use thiserror::Error;

/// Hex BLAKE3 of the DER certificate. 64 lowercase hex chars.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn from_cert(cert: &CertificateDer<'_>) -> Self {
        Self(blake3::hash(cert.as_ref()).to_hex().to_string())
    }

    pub fn parse(s: &str) -> Result<Self, IdentityError> {
        if s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Ok(Self(s.to_owned()))
        } else {
            Err(IdentityError::BadDeviceId)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_bytes32(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        hex::decode_to_slice(&self.0, &mut out).expect("DeviceId is validated hex");
        out
    }

    /// Short form safe for logs (identity is not secret, but keep logs compact).
    pub fn short(&self) -> &str {
        &self.0[..8]
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceId({}…)", self.short())
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("certificate generation failed: {0}")]
    Generate(#[from] rcgen::Error),
    #[error("identity storage error: {0}")]
    Io(#[from] io::Error),
    #[error("stored identity is corrupt")]
    Corrupt,
    #[error("malformed device id")]
    BadDeviceId,
}

/// The local device's long-lived identity.
pub struct Identity {
    cert: CertificateDer<'static>,
    key_pkcs8: Vec<u8>,
    id: DeviceId,
}

impl Identity {
    pub fn generate() -> Result<Self, IdentityError> {
        let ck = rcgen::generate_simple_self_signed(vec!["bolt.local".to_string()])?;
        Self::from_parts(
            ck.cert.der().clone().into_owned(),
            ck.signing_key.serialize_der(),
        )
    }

    fn from_parts(
        cert: CertificateDer<'static>,
        key_pkcs8: Vec<u8>,
    ) -> Result<Self, IdentityError> {
        let id = DeviceId::from_cert(&cert);
        Ok(Self {
            cert,
            key_pkcs8,
            id,
        })
    }

    /// Load the identity from `dir`, creating and persisting it on first run.
    ///
    /// The private key is written with owner-only permissions on Unix. On
    /// Windows it relies on the per-user profile ACL of `dir`; wrapping it with
    /// DPAPI / Android Keystore is tracked in docs/security.md.
    pub fn load_or_create(dir: &Path) -> Result<Self, IdentityError> {
        let cert_path = dir.join("identity.cert.der");
        let key_path = dir.join("identity.key.der");
        match (fs::read(&cert_path), fs::read(&key_path)) {
            (Ok(cert), Ok(key)) => {
                if cert.is_empty() || key.is_empty() {
                    return Err(IdentityError::Corrupt);
                }
                Self::from_parts(CertificateDer::from(cert), key)
            }
            (Err(a), Err(b))
                if a.kind() == io::ErrorKind::NotFound && b.kind() == io::ErrorKind::NotFound =>
            {
                fs::create_dir_all(dir)?;
                let ident = Self::generate()?;
                write_private(&key_path, &ident.key_pkcs8)?;
                write_private(&cert_path, ident.cert.as_ref())?;
                Ok(ident)
            }
            (Err(e), _) | (_, Err(e)) => Err(e.into()),
        }
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.id
    }

    pub fn cert_chain(&self) -> Vec<CertificateDer<'static>> {
        vec![self.cert.clone()]
    }

    pub fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_pkcs8.clone()))
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("id", &self.id)
            .field("key", &"<redacted>")
            .finish()
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        fs::write(&tmp, bytes)?;
    }
    fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_and_reloads_same_identity() {
        let dir = tempfile::tempdir().unwrap();
        let a = Identity::load_or_create(dir.path()).unwrap();
        let b = Identity::load_or_create(dir.path()).unwrap();
        assert_eq!(a.device_id(), b.device_id());
    }

    #[test]
    fn distinct_installs_have_distinct_ids() {
        assert_ne!(
            Identity::generate().unwrap().device_id(),
            Identity::generate().unwrap().device_id()
        );
    }

    #[test]
    fn device_id_parse_is_strict() {
        assert!(DeviceId::parse(&"a".repeat(64)).is_ok());
        assert!(DeviceId::parse(&"A".repeat(64)).is_err());
        assert!(DeviceId::parse("abc").is_err());
    }

    #[test]
    fn debug_never_prints_key() {
        let s = format!("{:?}", Identity::generate().unwrap());
        assert!(s.contains("redacted"));
    }
}
