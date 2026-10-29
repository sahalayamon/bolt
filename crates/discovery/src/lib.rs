//! Local-network discovery: mDNS/DNS-SD advertising and browsing, a device
//! registry with duplicate suppression and expiry, and manual endpoints.

mod mdns;
mod registry;

pub use mdns::{Advertisement, Discovery, DiscoveryError};
pub use registry::{DiscoveredDevice, Registry};

use std::net::{IpAddr, SocketAddr};

/// Default QUIC port. Falls back to an ephemeral port if taken (the real port is advertised).
pub const DEFAULT_PORT: u16 = 47600;

/// Parse a user-typed endpoint: `192.168.1.5`, `192.168.1.5:47600`, `[fe80::1]:47600`.
pub fn parse_manual_endpoint(input: &str) -> Result<SocketAddr, String> {
    let s = input.trim();
    if let Ok(sa) = s.parse::<SocketAddr>() {
        return Ok(sa);
    }
    if let Ok(ip) = s.trim_matches(|c| c == '[' || c == ']').parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, DEFAULT_PORT));
    }
    Err(format!("'{s}' is not a valid IP address or ip:port"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_endpoints() {
        assert_eq!(
            parse_manual_endpoint("192.168.1.5").unwrap().port(),
            DEFAULT_PORT
        );
        assert_eq!(
            parse_manual_endpoint(" 10.0.0.2:1234 ").unwrap().port(),
            1234
        );
        assert!(parse_manual_endpoint("[fe80::1]:99").unwrap().is_ipv6());
        assert!(parse_manual_endpoint("::1").unwrap().is_ipv6());
        assert!(parse_manual_endpoint("my-phone.local").is_err());
        assert!(parse_manual_endpoint("").is_err());
    }
}
