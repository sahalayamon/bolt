use crate::identity::DeviceId;
use std::{collections::HashMap, sync::Mutex};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedDevice {
    pub id: DeviceId,
    pub name: String,
    pub paired_at_unix: i64,
    pub last_seen_unix: Option<i64>,
}

#[derive(Debug, Error)]
#[error("trust store error: {0}")]
pub struct TrustError(pub String);

/// Persistent set of paired devices. `transfer-store` provides the SQLite
/// implementation; [`MemoryTrustStore`] is for tests and ephemeral use.
pub trait TrustStore: Send + Sync {
    fn is_trusted(&self, id: &DeviceId) -> bool;
    fn trust(&self, device: TrustedDevice) -> Result<(), TrustError>;
    fn revoke(&self, id: &DeviceId) -> Result<(), TrustError>;
    fn list(&self) -> Result<Vec<TrustedDevice>, TrustError>;
}

#[derive(Default)]
pub struct MemoryTrustStore(Mutex<HashMap<DeviceId, TrustedDevice>>);

impl MemoryTrustStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<DeviceId, TrustedDevice>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl TrustStore for MemoryTrustStore {
    fn is_trusted(&self, id: &DeviceId) -> bool {
        self.lock().contains_key(id)
    }
    fn trust(&self, device: TrustedDevice) -> Result<(), TrustError> {
        self.lock().insert(device.id.clone(), device);
        Ok(())
    }
    fn revoke(&self, id: &DeviceId) -> Result<(), TrustError> {
        self.lock().remove(id);
        Ok(())
    }
    fn list(&self) -> Result<Vec<TrustedDevice>, TrustError> {
        Ok(self.lock().values().cloned().collect())
    }
}
