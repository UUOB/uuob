//! What the native screens rely on: start UUOB, call methods, hear changes.
//! These are the functions UniFFI hands to Swift and Kotlin.

use std::sync::mpsc;
use std::time::Duration;

use serde_json::Value as Json;
use uuob_app::ffi::{UuobError, call, next_event, start, version};

/// Waits like Swift or Kotlin would: from outside UUOB's runtime.
fn wait<T>(fut: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(fut)
}

fn ok(method: &str, params: &str) -> Json {
    serde_json::from_str(&wait(call(method.into(), params.into())).unwrap()).unwrap()
}

/// Waits up to 10 s for an event of `kind`, skipping others.
fn wait_for(events: &mpsc::Receiver<Json>, kind: &str) -> Json {
    loop {
        let e = events.recv_timeout(Duration::from_secs(10)).unwrap_or_else(|_| panic!("no {kind} event"));
        if e["type"] == kind {
            return e;
        }
    }
}

#[test]
fn start_call_and_follow_changes() {
    let dir = tempfile::tempdir().unwrap();
    // Not started yet: calls say so instead of failing badly.
    let Err(UuobError::Failed { code, .. }) = wait(call("status".into(), String::new())) else { panic!() };
    assert_eq!(code, "unavailable");

    let data_dir = dir.path().to_string_lossy().to_string();
    wait(start(data_dir.clone())).unwrap();
    // The window and the background service may both start it: once is enough.
    wait(start(data_dir)).unwrap();

    assert!(ok("status", "")["device_id"].is_string());
    assert_eq!(ok("version", ""), version());
    let Err(UuobError::Failed { code, detail }) = wait(call("set_device_name".into(), "{}".into())) else { panic!() };
    assert_eq!(code, "invalid");
    assert!(!detail.is_empty());

    // The screen's loop hears changes made through the bridge.
    let (tx, events) = mpsc::channel();
    std::thread::spawn(move || {
        while tx.send(serde_json::from_str(&wait(next_event())).unwrap()).is_ok() {}
    });
    ok("set_device_name", r#"{"name": "Test computer"}"#);
    wait_for(&events, "network_changed");
    assert_eq!(ok("devices", "")[0]["name"], "Test computer");
}
