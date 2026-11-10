//! Test support: the built C# worker, shared by every test binary that needs it.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The worker assembly for tests. `SUDSY_CSHARP_WORKER` wins (the speed measurements use a Release
/// build); otherwise the Debug build, built once per process under a file lock so concurrent test
/// processes (nextest, parallel gates) never compile the same project at once (CS2012).
pub fn built_worker() -> PathBuf {
    static DLL: OnceLock<PathBuf> = OnceLock::new();
    DLL.get_or_init(|| {
        if let Some(path) = std::env::var_os(crate::WORKER_ENV) {
            return std::path::absolute(path).expect("worker path");
        }
        let project =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../workers/csharp/Sudsy.Worker");
        std::fs::create_dir_all(project.join("obj")).expect("worker obj dir");
        let lock = std::fs::File::create(project.join("obj/test-build.lock")).expect("lock file");
        lock.lock().expect("build lock");
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
