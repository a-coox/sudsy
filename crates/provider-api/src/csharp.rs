//! The C# provider's out-of-process half. Moves to `lang-csharp` in M4.

use crate::out_of_process::OutOfProcessProvider;
use crate::provider::FileMatcher;
use crate::proxy::WorkerConfig;
use core_syntax::{LanguageId, grammars};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Environment variable naming the worker assembly (`Sudsy.Worker.dll`).
pub const WORKER_ENV: &str = "SUDSY_CSHARP_WORKER";

/// Finds the worker assembly: `$SUDSY_CSHARP_WORKER`, else `csharp-worker/Sudsy.Worker.dll`
/// next to the executable.
pub fn locate_worker() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os(WORKER_ENV) {
        return Ok(PathBuf::from(p));
    }
    let beside = std::env::current_exe().ok().and_then(|exe| {
        exe.parent()
            .map(|d| d.join("csharp-worker").join("Sudsy.Worker.dll"))
    });
    match beside {
        Some(p) if p.exists() => Ok(p),
        _ => Err(format!(
            "C# worker not found: set {WORKER_ENV} to Sudsy.Worker.dll (build it with `dotnet build workers/csharp`)"
        )),
    }
}

/// A C# provider that runs the framework-dependent worker through the `dotnet` host.
pub fn provider(worker_dll: &Path) -> OutOfProcessProvider {
    let grammar = grammars::csharp();
    let command: Vec<OsString> = vec!["dotnet".into(), worker_dll.into()];
    OutOfProcessProvider::new(
        LanguageId::CSHARP,
        grammar,
        vec![FileMatcher::extension("cs")],
        WorkerConfig::new(command, grammar.hash()),
    )
}
