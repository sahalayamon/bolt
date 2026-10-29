//! Default-gateway lookup. When this computer joins a phone's hotspot, the gateway *is* the
//! phone, and mDNS between the two often doesn't work, so the app probes it directly.

use std::net::Ipv4Addr;

/// IPv4 default gateways, best (lowest metric) first. Empty if they can't be determined.
pub fn default_gateways() -> Vec<Ipv4Addr> {
    #[cfg(windows)]
    let out = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("route")
            .args(["print", "-4", "0.0.0.0"])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
    };
    #[cfg(not(windows))]
    let out = std::process::Command::new("ip")
        .args(["route", "show", "default"])
        .output();
    match out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            #[cfg(windows)]
            let gws = parse_windows_route(&text);
            #[cfg(not(windows))]
            let gws = parse_ip_route(&text);
            gws
        }
        Err(_) => Vec::new(),
    }
}

/// Parses `route print -4` output: `0.0.0.0  0.0.0.0  <gateway>  <interface>  <metric>`.
#[allow(dead_code)]
pub fn parse_windows_route(text: &str) -> Vec<Ipv4Addr> {
    let mut rows: Vec<(u32, Ipv4Addr)> = Vec::new();
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() >= 5 && t[0] == "0.0.0.0" && t[1] == "0.0.0.0" {
            if let (Ok(gw), Ok(metric)) = (t[2].parse::<Ipv4Addr>(), t[4].parse::<u32>()) {
                rows.push((metric, gw));
            }
        }
    }
    rows.sort();
    let mut seen = Vec::new();
    for (_, gw) in rows {
        if !seen.contains(&gw) {
            seen.push(gw);
        }
    }
    seen
}

/// Parses `ip route show default`: `default via <gateway> dev <if> ... metric <n>`.
#[allow(dead_code)]
pub fn parse_ip_route(text: &str) -> Vec<Ipv4Addr> {
    let mut rows: Vec<(u32, Ipv4Addr)> = Vec::new();
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.first() != Some(&"default") {
            continue;
        }
        let gw = t
            .iter()
            .position(|x| *x == "via")
            .and_then(|i| t.get(i + 1))
            .and_then(|g| g.parse::<Ipv4Addr>().ok());
        let metric = t
            .iter()
            .position(|x| *x == "metric")
            .and_then(|i| t.get(i + 1))
            .and_then(|m| m.parse::<u32>().ok())
            .unwrap_or(0);
        if let Some(gw) = gw {
            rows.push((metric, gw));
        }
    }
    rows.sort();
    rows.into_iter().map(|(_, g)| g).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_route_picks_lowest_metric_first_and_skips_on_link() {
        let text = "Active Routes:\n  Network Destination        Netmask          Gateway       Interface  Metric\n          0.0.0.0          0.0.0.0     10.20.234.208     10.20.234.65     50\n          0.0.0.0          0.0.0.0     192.168.1.1     192.168.1.9     25\n          0.0.0.0          0.0.0.0         On-link     10.0.0.2     5\n        127.0.0.0        255.0.0.0         On-link         127.0.0.1    331\n";
        assert_eq!(
            parse_windows_route(text),
            vec![
                Ipv4Addr::new(192, 168, 1, 1),
                Ipv4Addr::new(10, 20, 234, 208)
            ]
        );
    }

    #[test]
    fn linux_route() {
        let text = "default via 192.168.43.1 dev wlan0 proto dhcp metric 600\ndefault via 10.0.0.1 dev eth0 metric 100\n";
        assert_eq!(
            parse_ip_route(text),
            vec![Ipv4Addr::new(10, 0, 0, 1), Ipv4Addr::new(192, 168, 43, 1)]
        );
    }
}
