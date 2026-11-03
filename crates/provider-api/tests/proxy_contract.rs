//! Proxy behavior against a fake worker: handshake, refusal, crash/restart, hang.

mod common;

use common::fake_proxy;
use provider_api::ProxyError;
use provider_api::protocol::ParseRequest;
use std::time::Duration;

fn request(text: &str) -> ParseRequest {
    ParseRequest {
        file_id: "f".into(),
        version: 3,
        text: text.into(),
        language_version: None,
        preprocessor_symbols: vec![],
    }
}

#[test]
fn healthy_worker_parses_and_shuts_down() {
    let proxy = fake_proxy("echo", None, Duration::from_secs(10));
    let parsed = proxy.parse(request("abc")).unwrap();
    assert_eq!((parsed.file_id.as_str(), parsed.version), ("f", 3));
    assert!(proxy.parse(request("again")).is_ok());
    proxy.shutdown();
    assert!(matches!(
        proxy.parse(request("x")),
        Err(ProxyError::Unavailable(_))
    ));
}

#[test]
fn refused_handshake_is_final() {
    let proxy = fake_proxy("refuse", None, Duration::from_secs(10));
    let e = proxy.parse(request("abc")).unwrap_err();
    assert!(e.to_string().contains("refused the handshake"), "{e}");
    // No restart for a version mismatch: it would fail the same way.
    assert!(
        proxy
            .parse(request("abc"))
            .unwrap_err()
            .to_string()
            .contains("refused the handshake")
    );
}

#[test]
fn mismatched_hello_reply_is_rejected() {
    let proxy = fake_proxy("bad-hash", None, Duration::from_secs(10));
    let e = proxy.parse(request("abc")).unwrap_err();
    assert!(e.to_string().contains("version mismatch"), "{e}");
}

#[test]
fn missing_executable_is_a_missing_runtime() {
    let config = provider_api::WorkerConfig::new(vec!["definitely-not-a-real-program".into()], "h");
    let proxy = provider_api::WorkerProxy::new(config);
    for _ in 0..2 {
        let e = proxy.parse(request("x")).unwrap_err();
        assert!(
            matches!(&e, ProxyError::RuntimeMissing(r) if r.contains("definitely-not-a-real-program")),
            "{e}"
        );
    }
}

#[test]
fn host_without_its_framework_is_a_missing_runtime() {
    let proxy = fake_proxy("missing-framework", None, Duration::from_secs(10));
    let e = proxy.parse(request("x")).unwrap_err();
    assert!(
        matches!(&e, ProxyError::RuntimeMissing(r) if r.contains("'Microsoft.NETCore.App'")),
        "{e}"
    );
}

#[test]
fn crash_is_retried_once_on_a_fresh_worker() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("crashed");
    let proxy = fake_proxy("crash-once", Some(&marker), Duration::from_secs(10));
    assert!(
        proxy.parse(request("abc")).is_ok(),
        "first crash must be absorbed by a restart"
    );
    assert!(marker.exists());
    assert!(proxy.parse(request("abc")).is_ok());
}

#[test]
fn a_reply_gives_the_restart_allowance_back() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("crashed");
    let proxy = fake_proxy("crash-once", Some(&marker), Duration::from_secs(10));
    for _ in 0..3 {
        // Each parse finds the worker crashing once and is saved by its one restart; without the
        // reset the second round would find the allowance spent.
        std::fs::remove_file(&marker).ok();
        assert!(proxy.parse(request("abc")).is_ok());
    }
}

#[test]
fn second_crash_makes_the_worker_unavailable() {
    let proxy = fake_proxy("crash-on-parse", None, Duration::from_secs(10));
    let e = proxy.parse(request("abc")).unwrap_err();
    assert!(matches!(e, ProxyError::Unavailable(_)));
    assert!(e.to_string().contains("failed after 1 restart"), "{e}");
    assert!(matches!(
        proxy.parse(request("abc")),
        Err(ProxyError::Unavailable(_))
    ));
}

#[test]
fn hang_is_treated_like_a_crash() {
    let proxy = fake_proxy("hang-on-parse", None, Duration::from_millis(500));
    let started = std::time::Instant::now();
    let e = proxy.parse(request("abc")).unwrap_err();
    assert!(e.to_string().contains("did not answer"), "{e}");
    assert!(started.elapsed() < Duration::from_secs(30));
}
