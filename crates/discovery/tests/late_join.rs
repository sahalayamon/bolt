//! Real multicast test (needs a network interface with multicast; run with `--ignored`):
//! a device that starts *after* another is already browsing must show up without restarting either.

use bolt_discovery::{Advertisement, Discovery};
use bolt_protocol::DeviceKind;
use bolt_security::Identity;
use std::time::Duration;

fn ad(name: &str, port: u16) -> Advertisement {
    Advertisement {
        id: Identity::generate().unwrap().device_id().clone(),
        name: name.into(),
        kind: DeviceKind::Desktop,
        port,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "uses real mDNS multicast on this machine"]
async fn late_joiner_is_found_without_restarting_the_first_device() {
    let first = Discovery::start(&ad("first", 41001)).unwrap();
    let mut rx = first.devices();
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(!rx.borrow().iter().any(|d| d.name == "second"));

    let second = Discovery::start(&ad("second", 41002)).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(40);
    loop {
        if rx.borrow().iter().any(|d| d.name == "second") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "late joiner never appeared"
        );
        let _ = tokio::time::timeout(Duration::from_secs(2), rx.changed()).await;
    }
    second.stop();
    first.stop();
}
