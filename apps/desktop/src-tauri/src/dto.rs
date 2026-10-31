//! Serializable shapes shared with the TypeScript UI (camelCase JSON).
//! The TypeScript mirror lives in `src/lib/types.ts`.

use bolt_discovery::DiscoveredDevice;
use bolt_protocol::DeviceKind;
use bolt_security::TrustedDevice;
use serde::{Deserialize, Serialize};

pub fn kind_str(k: DeviceKind) -> &'static str {
    match k {
        DeviceKind::Desktop => "desktop",
        DeviceKind::Phone => "phone",
        DeviceKind::Tablet => "tablet",
        DeviceKind::Unknown => "unknown",
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDto {
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub kind: &'static str,
    pub addrs: Vec<String>,
    pub trusted: bool,
}

impl DeviceDto {
    pub fn from_discovered(d: &DiscoveredDevice, trusted: bool) -> Self {
        Self {
            id: d.id.to_string(),
            short_id: d.id.short().to_string(),
            name: d.name.clone(),
            kind: kind_str(d.kind),
            addrs: d.addrs.iter().map(|a| a.to_string()).collect(),
            trusted,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrustedDto {
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub paired_at: i64,
    pub last_seen: Option<i64>,
    pub online: bool,
}

impl TrustedDto {
    pub fn from_trusted(t: &TrustedDevice, online: bool) -> Self {
        Self {
            id: t.id.to_string(),
            short_id: t.id.short().to_string(),
            name: t.name.clone(),
            paired_at: t.paired_at_unix,
            last_seen: t.last_seen_unix,
            online,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransferDto {
    pub id: String,
    /// "sent" | "received"
    pub direction: &'static str,
    pub peer_id: String,
    pub peer_name: String,
    /// connecting | awaiting_approval | transferring | completed | failed | cancelled | rejected
    pub state: &'static str,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: u32,
    pub files_total: u32,
    pub current_file: Option<String>,
    pub speed_bps: f64,
    pub eta_secs: Option<f64>,
    pub error: Option<String>,
    pub title: String,
    pub started_unix: i64,
    pub finished_unix: Option<i64>,
    pub can_retry: bool,
}

impl TransferDto {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            "completed" | "failed" | "cancelled" | "rejected"
        )
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OfferFileDto {
    pub path: String,
    pub size: u64,
    /// "file" | "text"
    pub kind: &'static str,
}

/// Text or links that arrived in a finished transfer. Untrusted: the UI shows it as plain text.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TextReceivedDto {
    pub transfer_id: String,
    pub peer_name: String,
    pub texts: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OfferDto {
    pub id: String,
    pub peer_id: String,
    pub peer_short_id: String,
    pub peer_name: String,
    pub peer_kind: &'static str,
    pub trusted: bool,
    pub total_bytes: u64,
    pub free_space: Option<u64>,
    pub file_count: usize,
    /// At most the first 200 files; `file_count` is the real number.
    pub files: Vec<OfferFileDto>,
    pub download_dir: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PairRequestDto {
    pub id: String,
    pub peer_id: String,
    pub peer_short_id: String,
    pub peer_name: String,
    pub code: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PairingInfoDto {
    pub uri: String,
    pub qr_svg: String,
    pub expires_unix: u64,
    pub address: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDto {
    pub transfer_id: String,
    pub direction: &'static str,
    pub peer_id: String,
    pub peer_name: String,
    pub status: &'static str,
    pub file_count: u32,
    pub title: String,
    pub total_bytes: u64,
    pub started_unix: i64,
    pub finished_unix: i64,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub device_id: String,
    pub short_id: String,
    pub device_name: String,
    pub port: u16,
    pub download_dir: String,
    pub discovery_active: bool,
    pub discovery_error: Option<String>,
    pub local_ip: String,
    pub version: &'static str,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub device_name: String,
    pub download_dir: String,
    pub auto_accept_trusted: bool,
    /// "dark" | "light" | "system"
    pub theme: String,
    /// "system" | "on" | "off" (on = always reduce motion)
    pub reduce_motion: String,
    pub notifications: bool,
    /// Launch Bolt (in the tray, window hidden) when you sign in to Windows. Mirrors the real OS setting.
    pub start_on_login: bool,
    /// Closing the window keeps Bolt running in the tray so it can still receive.
    pub run_in_background: bool,
    /// "Bolt" entry in the Explorer right-click menu. Mirrors the real registry state.
    pub context_menu: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SendTarget {
    /// A discovered/trusted device id (address is looked up from discovery).
    pub device_id: Option<String>,
    /// Manual `IP[:port]`.
    pub address: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    pub text: Option<String>,
    pub status: Option<String>,
    pub direction: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanEntryDto {
    pub name: String,
    pub size: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanDto {
    pub file_count: usize,
    pub total_bytes: u64,
    /// First 100 files, `/`-separated relative paths.
    pub files: Vec<ScanEntryDto>,
}
