#![allow(dead_code)]

use provider_api::{WorkerConfig, WorkerProxy};
use std::time::Duration;

pub fn fake_worker(mode: &str, marker: Option<&std::path::Path>) -> Vec<std::ffi::OsString> {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap().parent().unwrap().join("examples");
    let name = format!("fake_worker{}", std::env::consts::EXE_SUFFIX);
    let mut cmd: Vec<std::ffi::OsString> = vec![dir.join(name).into(), mode.into()];
    cmd.extend(marker.map(|m| m.into()));
    cmd
}

pub fn fake_proxy(mode: &str, marker: Option<&std::path::Path>, timeout: Duration) -> WorkerProxy {
    let mut config = WorkerConfig::new(fake_worker(mode, marker), "hash");
    config.request_timeout = timeout;
    config.handshake_timeout = Duration::from_secs(10);
    WorkerProxy::new(config)
}
