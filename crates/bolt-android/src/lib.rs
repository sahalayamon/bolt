//! JNI bridge between the Android app (Kotlin) and the Bolt Rust core.
//!
//! The surface is deliberately narrow: plain strings/ints in, JSON out. Every
//! asynchronous result (incoming offers, progress, completions) is pushed to a
//! single Kotlin callback object as one JSON document, so the Kotlin side has
//! one `when (type)` instead of many JNI signatures.
//!
//! SAF `content://` documents are never given to Rust as paths. Kotlin passes
//! the URI string; when the sender needs to read it, Rust calls back
//! `Callbacks.openFd(uri)`, receives a *detached* file descriptor, and takes
//! ownership of it as a `std::fs::File` (see `FileOpener` in transfer-core).

use bolt_protocol::{DeviceKind, ItemKind};
use bolt_security::{DeviceId, Identity, PairingUri, TrustStore};
use bolt_transfer_core::{
    finalize_items, link_test, pair_with, probe, send_files, EndpointTuning, FileOpener,
    FlashEndpoint, IncomingEvent, IncomingOffer, IncomingPair, LocalInfo, Progress,
    ProgressReporter, Receiver, ReceiverConfig, ReceiverHandle, SendItem, SendRequest, StageTimes,
    TransferConfig, TransferState,
};
use bolt_transfer_store::{Direction, HistoryEntry, HistoryQuery, Status, Store};
use jni::{
    objects::{GlobalRef, JClass, JObject, JString, JValue},
    sys::{jboolean, jint, jstring, JNI_TRUE},
    JNIEnv, JavaVM,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::File,
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(3)
            .enable_all()
            .thread_name("bolt-rt")
            .build()
            .expect("tokio runtime")
    })
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Handle to the Kotlin `Callbacks` object; callable from any Rust thread.
#[derive(Clone)]
struct Callbacks {
    vm: Arc<JavaVM>,
    obj: Arc<GlobalRef>,
}

impl std::fmt::Debug for Callbacks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Callbacks")
    }
}

impl Callbacks {
    fn emit(&self, v: Value) {
        let Ok(mut env) = self.vm.attach_current_thread() else {
            return;
        };
        let Ok(s) = env.new_string(v.to_string()) else {
            return;
        };
        let _ = env.call_method(
            self.obj.as_obj(),
            "onEvent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&s)],
        );
        let _ = env.exception_clear();
    }

    /// Ask Kotlin to open a document and hand over a detached fd.
    fn open_fd(&self, uri: &str) -> std::io::Result<File> {
        let err = |m: String| std::io::Error::other(m);
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| err(e.to_string()))?;
        let s = env.new_string(uri).map_err(|e| err(e.to_string()))?;
        let r = env.call_method(
            self.obj.as_obj(),
            "openFd",
            "(Ljava/lang/String;)I",
            &[JValue::Object(&s)],
        );
        let _ = env.exception_clear();
        let fd = r
            .and_then(|v| v.i())
            .map_err(|e| err(format!("openFd failed: {e}")))?;
        if fd < 0 {
            return Err(err(format!("could not open {uri}")));
        }
        #[cfg(unix)]
        {
            use std::os::fd::FromRawFd;
            // SAFETY: Kotlin returned a detached fd (ParcelFileDescriptor.detachFd) that
            // nothing else owns; we take ownership and close it on drop.
            Ok(unsafe { File::from_raw_fd(fd) })
        }
        #[cfg(not(unix))]
        {
            Err(err("fd handoff is only available on Android".into()))
        }
    }
}

#[derive(Debug)]
struct UriOpener {
    uri: String,
    cb: Callbacks,
}

impl FileOpener for UriOpener {
    fn open(&self) -> std::io::Result<File> {
        self.cb.open_fd(&self.uri)
    }
}

struct Core {
    cb: Callbacks,
    ep: FlashEndpoint,
    handle: ReceiverHandle,
    store: Arc<Store>,
    device_name: Mutex<String>,
    offers: Mutex<HashMap<String, IncomingOffer>>,
    pairs: Mutex<HashMap<String, IncomingPair>>,
    jobs: Mutex<HashMap<String, CancellationToken>>,
    /// Raw-multicast mDNS browse, in addition to the platform's NsdManager (see `bolt_discovery::Discovery::browse`).
    mdns: Mutex<Option<bolt_discovery::Discovery>>,
    mdns_cancel: CancellationToken,
}

fn core_slot() -> &'static Mutex<Option<Arc<Core>>> {
    static C: OnceLock<Mutex<Option<Arc<Core>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

fn core() -> Option<Arc<Core>> {
    lock(core_slot()).clone()
}

fn core_ep_buffers() -> (usize, usize) {
    core().map(|c| c.ep.udp_buffers()).unwrap_or((0, 0))
}

fn next_key(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(1);
    format!("{prefix}{}", N.fetch_add(1, Ordering::Relaxed))
}

fn stages_json(t: &StageTimes, p: &bolt_transfer_core::PathInfo) -> Value {
    let ms = |d: std::time::Duration| (d.as_secs_f64() * 1000.0).round() as u64;
    json!({
        "netMs": ms(t.net), "diskMs": ms(t.disk), "hashMs": ms(t.hash), "syncMs": ms(t.sync),
        "finalizeMs": ms(t.finalize), "calls": t.net_calls, "bytes": t.bytes,
        "text": t.to_string(), "path": p.to_string(),
        "rttMs": p.rtt_ms, "lossPct": p.loss_pct(), "mtu": p.mtu, "cwndKiB": p.cwnd_bytes / 1024,
    })
}

fn state_str(s: TransferState) -> &'static str {
    match s {
        TransferState::Connecting => "connecting",
        TransferState::AwaitingApproval => "awaiting_approval",
        TransferState::Transferring => "transferring",
        TransferState::Completed => "completed",
        TransferState::Failed => "failed",
        TransferState::Cancelled => "cancelled",
        TransferState::Rejected => "rejected",
    }
}

fn progress_json(job: &str, dir: &str, peer: &str, p: &Progress) -> Value {
    json!({
        "t": "progress", "job": job, "dir": dir, "peer": peer,
        "state": state_str(p.state), "done": p.bytes_done, "total": p.bytes_total,
        "filesDone": p.files_done, "filesTotal": p.files_total, "current": p.current_file,
    })
}

fn terminal(s: TransferState) -> bool {
    matches!(
        s,
        TransferState::Completed
            | TransferState::Failed
            | TransferState::Cancelled
            | TransferState::Rejected
    )
}

fn forward_progress(
    cb: Callbacks,
    job: String,
    dir: &'static str,
    peer: String,
    mut rx: tokio::sync::watch::Receiver<Progress>,
) {
    runtime().spawn(async move {
        loop {
            let p = rx.borrow_and_update().clone();
            cb.emit(progress_json(&job, dir, &peer, &p));
            if terminal(p.state) || rx.changed().await.is_err() {
                break;
            }
        }
    });
}

/// Short history title for a text item: a one-line preview, never the full content.
fn text_title(t: &str) -> String {
    let head: String = t.chars().take(200).collect();
    let flat = head.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut it = flat.chars();
    let short: String = it.by_ref().take(40).collect();
    if it.next().is_some() {
        format!("Text · {short}…")
    } else {
        format!("Text · {short}")
    }
}

fn status_of(e: &bolt_transfer_core::TransferError) -> Status {
    use bolt_transfer_core::TransferError as T;
    match e {
        T::Rejected(_) => Status::Rejected,
        e if e.is_cancel() => Status::Cancelled,
        _ => Status::Failed,
    }
}

async fn event_loop(core: Arc<Core>, mut events: tokio::sync::mpsc::Receiver<IncomingEvent>) {
    while let Some(ev) = events.recv().await {
        match ev {
            IncomingEvent::OfferReceived(o) => {
                let id = next_key("o");
                let files: Vec<Value> = o
                    .offer
                    .files
                    .iter()
                    .filter(|f| f.kind != ItemKind::Text)
                    .take(50)
                    .map(|f| json!({"path": f.path, "size": f.size}))
                    .collect();
                // text items are never listed as files: only their sizes are shown
                let texts: Vec<u64> = o
                    .offer
                    .files
                    .iter()
                    .filter(|f| f.kind == ItemKind::Text)
                    .map(|f| f.size)
                    .collect();
                let msg = json!({
                    "t": "offer", "offerId": id, "peerId": o.peer.id.to_string(),
                    "peerName": o.peer.name, "trusted": o.trusted,
                    "fileCount": o.offer.files.len(), "total": o.total_bytes,
                    "free": o.free_space, "files": files, "texts": texts,
                });
                lock(&core.offers).insert(id, o);
                core.cb.emit(msg);
            }
            IncomingEvent::PairRequested(p) => {
                let id = next_key("p");
                let msg = json!({
                    "t": "pair_request", "pairId": id,
                    "peerName": p.peer.name, "peerId": p.peer.id.to_string(), "code": p.code,
                });
                lock(&core.pairs).insert(id, p);
                core.cb.emit(msg);
            }
            IncomingEvent::TransferStarted {
                transfer_id,
                peer,
                progress,
                cancel,
            } => {
                lock(&core.jobs).insert(transfer_id.clone(), cancel);
                forward_progress(core.cb.clone(), transfer_id, "recv", peer.name, progress);
            }
            IncomingEvent::TransferFinished {
                transfer_id,
                peer,
                result,
            } => {
                lock(&core.jobs).remove(&transfer_id);
                let (status, err, files, bytes, title, msg) = match &result {
                    Ok(s) => (
                        Status::Completed,
                        None,
                        s.files,
                        s.bytes,
                        s.saved_paths
                            .first()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().into_owned())
                            .or_else(|| s.texts.first().map(|t| text_title(t)))
                            .unwrap_or_default(),
                        json!({
                            "t": "finished", "job": transfer_id, "dir": "recv", "ok": true,
                            "peer": peer.name, "files": s.files, "bytes": s.bytes,
                            "elapsedMs": s.elapsed.as_millis() as u64,
                            "stages": stages_json(&s.timings, &s.path),
                            "paths": s.saved_paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
                            "texts": s.texts,
                        }),
                    ),
                    Err(e) => (
                        status_of(e),
                        Some(e.to_string()),
                        0,
                        0,
                        String::new(),
                        json!({
                            "t": "finished", "job": transfer_id, "dir": "recv", "ok": false,
                            "peer": peer.name, "error": e.to_string(), "cancelled": e.is_cancel(),
                        }),
                    ),
                };
                let _ = core.store.record_transfer(&HistoryEntry {
                    transfer_id: transfer_id.clone(),
                    direction: Direction::Received,
                    peer_id: peer.id.to_string(),
                    peer_name: peer.name.clone(),
                    status,
                    file_count: files,
                    title,
                    total_bytes: bytes,
                    started_unix: now(),
                    finished_unix: now(),
                    error: err,
                });
                core.cb.emit(msg);
            }
        }
    }
}

// ---- JNI helpers ----------------------------------------------------------

fn jstr(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(Into::into).unwrap_or_default()
}

fn guarded(env: &mut JNIEnv, f: impl FnOnce() -> Value) -> jstring {
    let v = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| json!({"error": "internal error (panic in native core)"}));
    env.new_string(v.to_string())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

fn err(m: impl std::fmt::Display) -> Value {
    json!({"error": m.to_string()})
}

fn parse_addr(s: &str) -> Result<std::net::SocketAddr, String> {
    let s = s.trim();
    if let Ok(sa) = s.parse::<std::net::SocketAddr>() {
        return Ok(sa);
    }
    s.trim_matches(|c| c == '[' || c == ']')
        .parse::<std::net::IpAddr>()
        .map(|ip| std::net::SocketAddr::new(ip, 47600))
        .map_err(|_| format!("'{s}' is not a valid IP address or ip:port"))
}

fn local_info(core: &Core) -> LocalInfo {
    LocalInfo {
        name: lock(&core.device_name).clone(),
        kind: DeviceKind::Phone,
    }
}

/// Browse for peers with raw multicast and tell Kotlin whenever the set changes. This runs next to the platform's
/// NsdManager, which on a phone hosting a hotspot does not see the laptop that joined it; the two lists are merged.
fn start_mdns_browse(core: &Arc<Core>) {
    let disc = match bolt_discovery::Discovery::browse(core.ep.device_id().clone()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("bolt: mDNS browse unavailable: {e}");
            return;
        }
    };
    let mut rx = disc.devices();
    *lock(&core.mdns) = Some(disc);
    let (cb, cancel) = (core.cb.clone(), core.mdns_cancel.clone());
    runtime().spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                changed = rx.changed() => if changed.is_err() { break },
            }
            let list: Vec<Value> = rx
                .borrow()
                .iter()
                .map(|d| {
                    let addr = d
                        .addrs
                        .iter()
                        .find(|a| a.is_ipv4())
                        .or_else(|| d.addrs.iter().next());
                    json!({
                        "id": d.id.to_string(), "name": d.name,
                        "kind": match d.kind {
                            DeviceKind::Desktop => "desktop", DeviceKind::Phone => "phone",
                            DeviceKind::Tablet => "tablet", _ => "unknown",
                        },
                        "host": addr.map(|a| a.ip().to_string()).unwrap_or_default(),
                        "port": addr.map(|a| a.port()).unwrap_or(0),
                    })
                })
                .filter(|v| !v["host"].as_str().unwrap_or("").is_empty())
                .collect();
            cb.emit(json!({"t": "mdns", "devices": list}));
        }
    });
}

// ---- JNI entry points (class com.bolt.app.NativeBridge) --------------

/// Start the endpoint + receiver. Returns `{"id","port"}` or `{"error"}`.
#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_start<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    data_dir: JString<'l>,
    download_dir: JString<'l>,
    device_name: JString<'l>,
    port: jint,
    auto_accept: jboolean,
    tuning: JString<'l>,
    cb: JObject<'l>,
) -> jstring {
    let data_dir = jstr(&mut env, &data_dir);
    let download_dir = jstr(&mut env, &download_dir);
    let device_name = jstr(&mut env, &device_name);
    let tuning = EndpointTuning::parse(&jstr(&mut env, &tuning));
    let cbs = match (env.get_java_vm(), env.new_global_ref(&cb)) {
        (Ok(vm), Ok(obj)) => Some(Callbacks {
            vm: Arc::new(vm),
            obj: Arc::new(obj),
        }),
        _ => None,
    };
    guarded(&mut env, || {
        if core().is_some() {
            return err("already started");
        }
        let Some(cbs) = cbs else {
            return err("jni setup failed");
        };
        let _guard = runtime().enter();
        let data = std::path::Path::new(&data_dir);
        let ident = match Identity::load_or_create(data) {
            Ok(i) => Arc::new(i),
            Err(e) => return err(format!("identity: {e}")),
        };
        let store = match Store::open(&data.join("bolt.sqlite")) {
            Ok(s) => Arc::new(s),
            Err(e) => return err(format!("store: {e}")),
        };
        let bind = |p: u16| {
            FlashEndpoint::bind_with(
                ident.clone(),
                format!("0.0.0.0:{p}").parse().unwrap(),
                tuning,
            )
        };
        let ep = match bind(port as u16).or_else(|_| bind(0)) {
            Ok(e) => e,
            Err(e) => return err(format!("bind: {e}")),
        };
        let bound = ep.local_addr().map(|a| a.port()).unwrap_or(0);
        let rcfg = ReceiverConfig {
            download_dir: download_dir.into(),
            local: LocalInfo {
                name: device_name.clone(),
                kind: DeviceKind::Phone,
            },
            auto_accept_trusted: auto_accept == JNI_TRUE,
            max_concurrent_transfers: 2,
            transfer: TransferConfig::default(),
        };
        let trust: Arc<dyn TrustStore> = store.clone();
        let (handle, events) = Receiver::start(ep.clone(), rcfg, trust);
        let core = Arc::new(Core {
            cb: cbs,
            ep,
            handle,
            store,
            device_name: Mutex::new(device_name),
            offers: Default::default(),
            pairs: Default::default(),
            jobs: Default::default(),
            mdns: Mutex::new(None),
            mdns_cancel: CancellationToken::new(),
        });
        runtime().spawn(event_loop(core.clone(), events));
        start_mdns_browse(&core);
        let id = core.ep.device_id().to_string();
        *lock(core_slot()) = Some(core);
        let (rcv, snd) = core_ep_buffers();
        json!({"id": id, "port": bound, "rcvBuf": rcv, "sndBuf": snd})
    })
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_stop(_e: JNIEnv, _c: JClass) {
    if let Some(core) = lock(core_slot()).take() {
        core.handle.shutdown();
        for t in lock(&core.jobs).values() {
            t.cancel();
        }
        core.mdns_cancel.cancel();
        if let Some(d) = lock(&core.mdns).take() {
            d.stop();
        }
    }
}

/// Open a pairing window; returns `{"uri"}`. `addr` is the `ip:port` peers should dial.
#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_beginPairing<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    addr: JString<'l>,
    ttl_secs: jint,
) -> jstring {
    let addr = jstr(&mut env, &addr);
    guarded(&mut env, || match core() {
        Some(c) => {
            let uri = c.handle.begin_pairing(
                std::time::Duration::from_secs(ttl_secs.max(10) as u64),
                addr,
            );
            json!({"uri": uri.encode()})
        }
        None => err("not started"),
    })
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_endPairing(_e: JNIEnv, _c: JClass) {
    if let Some(c) = core() {
        c.handle.end_pairing();
    }
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_respondOffer<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    offer_id: JString<'l>,
    accept: jboolean,
    reason: JString<'l>,
) {
    let id = jstr(&mut env, &offer_id);
    let reason = jstr(&mut env, &reason);
    if let Some(c) = core() {
        let offer = lock(&c.offers).remove(&id);
        if let Some(o) = offer {
            if accept == JNI_TRUE {
                o.accept()
            } else if reason.is_empty() {
                o.reject("declined by user")
            } else {
                o.reject(reason)
            }
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_respondPair<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    pair_id: JString<'l>,
    approve: jboolean,
) {
    let id = jstr(&mut env, &pair_id);
    if let Some(c) = core() {
        let pair = lock(&c.pairs).remove(&id);
        if let Some(p) = pair {
            if approve == JNI_TRUE {
                p.approve()
            } else {
                p.deny()
            }
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_cancel<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    job: JString<'l>,
) {
    let job = jstr(&mut env, &job);
    if let Some(c) = core() {
        if let Some(t) = lock(&c.jobs).get(&job) {
            t.cancel();
        }
    }
}

/// Start sending. `items_json` = `[{"uri","rel","size","mtime"}]`; an entry `{"text": "..."}` is sent as a text/link item. Returns `{"job"}`
/// immediately; progress and the result arrive as events.
#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_send<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    addr: JString<'l>,
    expected_peer: JString<'l>,
    peer_name: JString<'l>,
    items_json: JString<'l>,
) -> jstring {
    let addr = jstr(&mut env, &addr);
    let expected = jstr(&mut env, &expected_peer);
    let peer_name = jstr(&mut env, &peer_name);
    let items_json = jstr(&mut env, &items_json);
    guarded(&mut env, || {
        let Some(core) = core() else {
            return err("not started");
        };
        let addr = match parse_addr(&addr) {
            Ok(a) => a,
            Err(e) => return err(e),
        };
        let expected_peer = if expected.is_empty() {
            None
        } else {
            match DeviceId::parse(&expected) {
                Ok(d) => Some(d),
                Err(e) => return err(format!("bad device id: {e}")),
            }
        };
        let parsed: Vec<Value> = match serde_json::from_str(&items_json) {
            Ok(v) => v,
            Err(e) => return err(format!("bad items: {e}")),
        };
        let mut items: Vec<SendItem> = Vec::with_capacity(parsed.len());
        let mut texts = 0;
        for v in &parsed {
            if let Some(t) = v["text"].as_str() {
                match SendItem::text(t, texts) {
                    Ok(i) => items.push(i),
                    Err(e) => return err(e),
                }
                texts += 1;
                continue;
            }
            let size = v["size"].as_u64().unwrap_or(0);
            let mtime = v["mtime"].as_i64().unwrap_or(0);
            items.push(SendItem::from_opener(
                v["rel"].as_str().unwrap_or("file").to_string(),
                size,
                format!("{size}-{mtime}"),
                Arc::new(UriOpener {
                    uri: v["uri"].as_str().unwrap_or_default().to_string(),
                    cb: core.cb.clone(),
                }),
            ));
        }
        let items = match finalize_items(items) {
            Ok(i) if !i.is_empty() => i,
            Ok(_) => return err("nothing to send"),
            Err(e) => return err(e),
        };
        let job = next_key("s");
        let cancel = CancellationToken::new();
        lock(&core.jobs).insert(job.clone(), cancel.clone());
        let (reporter, rx) = ProgressReporter::new();
        forward_progress(core.cb.clone(), job.clone(), "send", peer_name.clone(), rx);
        let title = match items[0].inline.as_deref() {
            Some(b) => text_title(&String::from_utf8_lossy(b)),
            None => items[0].rel_path.clone(),
        };
        let total: u64 = items.iter().map(|i| i.size).sum();
        let count = items.len() as u32;
        let local = local_info(&core);
        let job2 = job.clone();
        runtime().spawn(async move {
            let started = now();
            let req = SendRequest {
                addr,
                expected_peer,
                local,
                items,
                config: TransferConfig::default(),
            };
            let result = send_files(&core.ep, req, cancel, reporter).await;
            lock(&core.jobs).remove(&job2);
            let (status, e, id, name, peer_id) = match &result {
                Ok(s) => (
                    Status::Completed,
                    None,
                    s.transfer_id.clone(),
                    s.peer.name.clone(),
                    s.peer.id.to_string(),
                ),
                Err(e) => (
                    status_of(e),
                    Some(e.to_string()),
                    job2.clone(),
                    peer_name.clone(),
                    expected.clone(),
                ),
            };
            let _ = core.store.record_transfer(&HistoryEntry {
                transfer_id: id,
                direction: Direction::Sent,
                peer_id,
                peer_name: name,
                status,
                file_count: count,
                title,
                total_bytes: total,
                started_unix: started,
                finished_unix: now(),
                error: e,
            });
            core.cb.emit(match result {
                Ok(s) => json!({
                    "t": "finished", "job": job2, "dir": "send", "ok": true, "peer": s.peer.name, "transferId": s.transfer_id,
                    "files": s.files, "bytes": s.bytes, "elapsedMs": s.elapsed.as_millis() as u64,
                    "stages": stages_json(&s.timings, &s.path),
                }),
                Err(e) => json!({
                    "t": "finished", "job": job2, "dir": "send", "ok": false, "peer": peer_name,
                    "error": e.to_string(), "cancelled": e.is_cancel(),
                }),
            });
        });
        json!({"job": job})
    })
}

/// Pair with a device whose QR/URI we scanned. Emits `pair_code`, then `paired`/`pair_failed`.
#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_pairWith<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    uri: JString<'l>,
) -> jstring {
    let uri = jstr(&mut env, &uri);
    guarded(&mut env, || {
        let Some(core) = core() else {
            return err("not started");
        };
        let uri = match PairingUri::parse(&uri) {
            Ok(u) => u,
            Err(e) => return err(format!("not a Bolt pairing code: {e}")),
        };
        let addr = match parse_addr(&uri.addr) {
            Ok(a) => a,
            Err(e) => return err(e),
        };
        let job = next_key("pj");
        let j2 = job.clone();
        let local = local_info(&core);
        runtime().spawn(async move {
            let cb = core.cb.clone();
            let (cb2, j3) = (cb.clone(), j2.clone());
            let r = pair_with(
                &core.ep,
                addr,
                &uri,
                &local,
                &TransferConfig::default(),
                core.store.as_ref(),
                move |code| cb2.emit(json!({"t": "pair_code", "job": j3, "code": code})),
            )
            .await;
            cb.emit(match r {
                Ok(p) => json!({"t": "paired", "job": j2, "peerId": p.id.to_string(), "peerName": p.name}),
                Err(e) => json!({"t": "pair_failed", "job": j2, "error": e.to_string()}),
            });
        });
        json!({"job": job})
    })
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_trusted(
    mut env: JNIEnv,
    _c: JClass,
) -> jstring {
    guarded(&mut env, || {
        let Some(c) = core() else {
            return err("not started");
        };
        match c.store.list() {
            Ok(l) => json!({"devices": l.iter().map(|d| json!({
                "id": d.id.to_string(), "name": d.name, "pairedAt": d.paired_at_unix,
            })).collect::<Vec<_>>()}),
            Err(e) => err(e),
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_revoke<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    id: JString<'l>,
) {
    let id = jstr(&mut env, &id);
    if let (Some(c), Ok(id)) = (core(), DeviceId::parse(&id)) {
        let _ = c.store.revoke(&id);
    }
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_history<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    limit: jint,
) -> jstring {
    guarded(&mut env, || {
        let Some(c) = core() else {
            return err("not started");
        };
        match c.store.history(&HistoryQuery {
            limit: Some(limit.max(1) as u32),
            ..Default::default()
        }) {
            Ok(h) => json!({"items": h.iter().map(|e| json!({
                "id": e.transfer_id,
                "dir": if e.direction == Direction::Sent { "send" } else { "recv" },
                "peer": e.peer_name, "files": e.file_count, "title": e.title,
                "bytes": e.total_bytes, "at": e.finished_unix,
                "status": match e.status {
                    Status::Completed => "completed", Status::Failed => "failed",
                    Status::Cancelled => "cancelled", Status::Rejected => "rejected" },
                "error": e.error,
            })).collect::<Vec<_>>()}),
            Err(e) => err(e),
        }
    })
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_setAutoAccept(
    _e: JNIEnv,
    _c: JClass,
    on: jboolean,
) {
    if let Some(c) = core() {
        c.handle.set_auto_accept_trusted(on == JNI_TRUE);
    }
}

#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_setDeviceName<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    name: JString<'l>,
) {
    let name = jstr(&mut env, &name);
    if let Some(c) = core() {
        c.handle.set_device_name(name.clone());
        *lock(&c.device_name) = name;
    }
}

/// Is a Bolt receiver reachable at `addr` (and, if `peer_id` is non-empty, is it that device)?
/// Blocks up to ~2 s; call from a background thread.
#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_probe<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    addr: JString<'l>,
    peer_id: JString<'l>,
) -> jboolean {
    let addr = jstr(&mut env, &addr);
    let peer_id = jstr(&mut env, &peer_id);
    let reachable = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let core = core()?;
        let addr = parse_addr(&addr).ok()?;
        let expected = if peer_id.is_empty() {
            None
        } else {
            DeviceId::parse(&peer_id).ok()
        };
        Some(runtime().block_on(probe(
            &core.ep,
            addr,
            expected.as_ref(),
            std::time::Duration::from_secs(2),
        )))
    }))
    .ok()
    .flatten()
    .unwrap_or(false);
    reachable as jboolean
}

/// Network-only speed test (no disk, no hashing) in both directions. Blocks for the duration of the test
/// (tens of seconds at worst): call from a background thread. Returns JSON with MB/s per direction and QUIC path stats.
#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_linkTest<'l>(
    mut env: JNIEnv<'l>,
    _c: JClass<'l>,
    addr: JString<'l>,
    peer_id: JString<'l>,
    megabytes: jint,
) -> jstring {
    let addr = jstr(&mut env, &addr);
    let peer_id = jstr(&mut env, &peer_id);
    guarded(&mut env, || {
        let Some(core) = core() else {
            return err("not started");
        };
        let addr = match parse_addr(&addr) {
            Ok(a) => a,
            Err(e) => return err(e),
        };
        let expected = if peer_id.is_empty() {
            None
        } else {
            DeviceId::parse(&peer_id).ok()
        };
        let local = local_info(&core);
        let bytes = (megabytes.max(1) as u64) * 1_000_000;
        let r = runtime().block_on(link_test(
            &core.ep,
            addr,
            expected.as_ref(),
            &local,
            bytes,
            &TransferConfig::default(),
            CancellationToken::new(),
        ));
        match r {
            Ok(r) => json!({
                "connectMs": (r.connect.as_secs_f64() * 1000.0).round() as u64,
                "upMBps": r.up.mb_per_s(), "downMBps": r.down.mb_per_s(),
                "up": r.up.path.to_string(), "down": r.down.path.to_string(),
                "upLossPct": r.up.path.loss_pct(), "downLossPct": r.down.path.loss_pct(),
                "mtu": r.up.path.mtu, "rttMs": r.up.path.rtt_ms,
            }),
            Err(e) => err(e),
        }
    })
}

/// Delete the transfer history (the list only; received files are untouched). Returns the number of rows removed.
#[no_mangle]
pub extern "system" fn Java_com_bolt_app_NativeBridge_clearHistory(_e: JNIEnv, _c: JClass) -> jint {
    core()
        .and_then(|c| c.store.clear_history().ok())
        .map(|n| n as jint)
        .unwrap_or(0)
}
