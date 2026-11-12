#![allow(dead_code)]

use provider_api::{WorkerConfig, WorkerProxy};
use std::time::Duration;

pub fn fake_worker(mode: &str, marker: Option<&std::path::Path>) -> Vec<std::ffi::OsString> {
    // A bin, not an example: cargo tells tests where bins are; example paths vary by build-dir layout.
    let mut cmd: Vec<std::ffi::OsString> =
        vec![env!("CARGO_BIN_EXE_fake_worker").into(), mode.into()];
    cmd.extend(marker.map(|m| m.into()));
    cmd
}

pub fn fake_proxy(mode: &str, marker: Option<&std::path::Path>, timeout: Duration) -> WorkerProxy {
    let mut config = WorkerConfig::new(fake_worker(mode, marker), "hash");
    config.request_timeout = timeout;
    config.handshake_timeout = Duration::from_secs(10);
    WorkerProxy::new(config)
}
