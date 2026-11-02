#![allow(dead_code)]

use provider_api::{WorkerConfig, WorkerProxy};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Builds the C# worker once per test process and returns the assembly path.
pub fn worker_dll() -> PathBuf {
    static DLL: OnceLock<PathBuf> = OnceLock::new();
    DLL.get_or_init(|| {
        let project = repo_root().join("workers/csharp/Sudsy.Worker");
        let status = std::process::Command::new("dotnet")
            .args(["build", "-c", "Debug", "--nologo", "-v", "q"])
            .arg(&project)
            .status()
            .expect("dotnet is installed");
        assert!(status.success(), "dotnet build failed");
        project.join("bin/Debug/net10.0/Sudsy.Worker.dll")
    })
    .clone()
}

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
