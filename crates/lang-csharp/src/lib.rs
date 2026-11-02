//! The C# provider: the grammar's typed wrappers, the proxy to the Roslyn worker and the
//! formatter.

mod format;
#[rustfmt::skip]
pub mod generated;

pub use format::{
    BRACE_NEWLINE, CSharpFormatter, MEMBER_CHAIN_MIN_CALLS, TRAILING_COMMAS, option_schema,
};

use core_syntax::{LanguageId, grammars};
use provider_api::{FileMatcher, OutOfProcessProvider, WorkerConfig};
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

/// Environment variable with the largest number of worker processes (default: the number of
/// cores, at most four).
pub const WORKERS_ENV: &str = "SUDSY_CSHARP_WORKERS";

fn pool_size() -> usize {
    std::env::var(WORKERS_ENV)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(1, |n| n.get().min(MAX_WORKERS))
        })
}

const MAX_WORKERS: usize = 4;

/// A C# provider that runs the framework-dependent worker through the `dotnet` host.
pub fn provider(worker_dll: &Path) -> OutOfProcessProvider {
    let grammar = grammars::csharp();
    let command: Vec<OsString> = vec!["dotnet".into(), worker_dll.into()];
    let mut config = WorkerConfig::new(command, grammar.hash());
    config.pool_size = pool_size();
    // The worker is a batch process: start counting calls at once, so that its hot methods
    // reach optimized code in the first second instead of after the default 100 ms of quiet.
    config.env = vec![("DOTNET_TC_CallCountingDelayMs".into(), "0".into())];
    OutOfProcessProvider::new(
        LanguageId::CSHARP,
        grammar,
        vec![FileMatcher::extension("cs")],
        config,
    )
    .with_format(Box::new(CSharpFormatter), option_schema())
}
