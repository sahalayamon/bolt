use bolt_protocol::DeviceKind;
use bolt_security::DeviceId;
use std::{
    collections::{BTreeSet, HashMap},
    net::SocketAddr,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
    /// Every address the device is reachable on (multiple interfaces, v4+v6), merged.
    pub addrs: BTreeSet<SocketAddr>,
}

struct Entry {
    dev: DiscoveredDevice,
    /// mDNS fullnames that announced this device (one per interface/advertisement).
    sources: BTreeSet<String>,
    last_seen: Instant,
}

/// Tracks nearby devices. Keyed by cryptographic [`DeviceId`], so the same
/// device seen over several interfaces or address families is one entry.
pub struct Registry {
    me: DeviceId,
    ttl: Duration,
    devices: HashMap<DeviceId, Entry>,
}

impl Registry {
    pub fn new(me: DeviceId, ttl: Duration) -> Self {
        Self {
            me,
            ttl,
            devices: HashMap::new(),
        }
    }

    /// Insert or refresh. Returns true if the visible set changed.
    pub fn upsert(&mut self, source: &str, dev: DiscoveredDevice, now: Instant) -> bool {
        if dev.id == self.me || dev.addrs.is_empty() {
            return false;
        }
        match self.devices.get_mut(&dev.id) {
            Some(e) => {
                let before = e.dev.clone();
                e.sources.insert(source.to_owned());
                e.dev.name = dev.name;
                e.dev.kind = dev.kind;
                e.dev.addrs.extend(dev.addrs);
                e.last_seen = now;
                e.dev != before
            }
            None => {
                let sources = BTreeSet::from([source.to_owned()]);
                self.devices.insert(
                    dev.id.clone(),
                    Entry {
                        dev,
                        sources,
                        last_seen: now,
                    },
                );
                true
            }
        }
    }

    /// An announcement named `source` went away. The device disappears only when
    /// no other announcement still advertises it.
    pub fn remove_source(&mut self, source: &str) -> bool {
        let mut changed = false;
        self.devices.retain(|_, e| {
            if e.sources.remove(source) && e.sources.is_empty() {
                changed = true;
                false
            } else {
                true
            }
        });
        changed
    }

    /// Drop devices not refreshed within the TTL (covers missed goodbye packets).
    pub fn expire(&mut self, now: Instant) -> usize {
        let before = self.devices.len();
        let ttl = self.ttl;
        self.devices
            .retain(|_, e| now.duration_since(e.last_seen) < ttl);
        before - self.devices.len()
    }

    pub fn snapshot(&self) -> Vec<DiscoveredDevice> {
        let mut v: Vec<_> = self.devices.values().map(|e| e.dev.clone()).collect();
        v.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(c: char) -> DeviceId {
        DeviceId::parse(&c.to_string().repeat(64)).unwrap()
    }
    fn dev(c: char, name: &str, addr: &str) -> DiscoveredDevice {
        DiscoveredDevice {
            id: id(c),
            name: name.into(),
            kind: DeviceKind::Phone,
            addrs: BTreeSet::from([addr.parse().unwrap()]),
        }
    }

    #[test]
    fn same_device_on_two_interfaces_is_one_entry() {
        let mut r = Registry::new(id('0'), Duration::from_secs(60));
        let t = Instant::now();
        assert!(r.upsert("wifi", dev('a', "Pixel", "192.168.1.5:1"), t));
        assert!(r.upsert("hotspot", dev('a', "Pixel", "10.0.0.5:1"), t));
        let s = r.snapshot();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].addrs.len(), 2);
        assert!(
            !r.upsert("wifi", dev('a', "Pixel", "192.168.1.5:1"), t),
            "no change -> no UI churn"
        );
    }

    #[test]
    fn never_lists_self_or_addressless() {
        let mut r = Registry::new(id('0'), Duration::from_secs(60));
        assert!(!r.upsert("x", dev('0', "me", "1.1.1.1:1"), Instant::now()));
        let mut d = dev('a', "n", "1.1.1.1:1");
        d.addrs.clear();
        assert!(!r.upsert("x", d, Instant::now()));
        assert!(r.snapshot().is_empty());
    }

    #[test]
    fn removal_waits_for_last_source() {
        let mut r = Registry::new(id('0'), Duration::from_secs(60));
        let t = Instant::now();
        r.upsert("a", dev('a', "Pixel", "1.1.1.1:1"), t);
        r.upsert("b", dev('a', "Pixel", "2.2.2.2:1"), t);
        assert!(!r.remove_source("a"));
        assert_eq!(r.snapshot().len(), 1);
        assert!(r.remove_source("b"));
        assert!(r.snapshot().is_empty());
    }

    #[test]
    fn stale_devices_expire() {
        let mut r = Registry::new(id('0'), Duration::from_secs(30));
        let t = Instant::now();
        r.upsert("a", dev('a', "Old", "1.1.1.1:1"), t);
        r.upsert(
            "b",
            dev('b', "Fresh", "1.1.1.2:1"),
            t + Duration::from_secs(25),
        );
        assert_eq!(r.expire(t + Duration::from_secs(40)), 1);
        assert_eq!(r.snapshot()[0].name, "Fresh");
    }

    #[test]
    fn name_change_is_a_change() {
        let mut r = Registry::new(id('0'), Duration::from_secs(60));
        let t = Instant::now();
        r.upsert("a", dev('a', "Pixel", "1.1.1.1:1"), t);
        assert!(r.upsert("a", dev('a', "Sahal's Pixel", "1.1.1.1:1"), t));
    }
}
