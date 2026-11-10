//! The C# provider: the grammar's typed wrappers, the proxy to the Roslyn worker and the
//! formatter.

mod format;
mod rules;
pub mod testing;
mod using_sort;
core_syntax::include_generated!();

pub use format::{
    BRACE_NEWLINE, CSharpFormatter, MEMBER_CHAIN_MIN_CALLS, PREPROCESSOR_SYMBOLS, TRAILING_COMMAS,
    option_schema,
};

use core_semantics::BehaviorSummaries;
use core_syntax::grammars;
use provider_api::{FileMatcher, OutOfProcessProvider, WorkerConfig};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Environment variable naming the worker assembly (`Sudsy.Worker.dll`).
pub const WORKER_ENV: &str = "SUDSY_CSHARP_WORKER";

/// Finds the worker assembly: `$SUDSY_CSHARP_WORKER`, else `csharp-worker/Sudsy.Worker.dll`
/// next to the executable.
pub fn locate_worker() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os(WORKER_ENV) {
        // Absolute, because the worker does not run in the caller's directory.
        return std::path::absolute(&p).map_err(|e| format!("{WORKER_ENV}: {e}"));
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

/// Parsed by each prestarted worker, so that the parser is compiled before the first request.
const WARMUP_SOURCE: &str = r#"using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;

namespace Warmup;

[Serializable]
public sealed record Point(int X, int Y);

public interface IShape<T> where T : struct { T Area { get; } }

public abstract class Base<T> : IShape<double>
{
    private readonly Dictionary<string, List<int>> _map = new();
    public event EventHandler? Changed;
    public double Area => 3.14 * Math.Pow(2, 2);
    public T? Value { get; set; }

    public async Task<int> RunAsync(string[] args, CancellationToken token = default)
    {
        var items = args.Where(a => a.Length > 0).Select((a, i) => $"{i}: {a}").ToList();
        foreach (var item in items) { Console.WriteLine(item); }
        for (int i = 0; i < 10; i++) { if (i % 2 == 0) continue; else break; }
        while (items.Count > 0) items.RemoveAt(0);
        try { await Task.Delay(1, token); }
        catch (OperationCanceledException ex) when (ex.CancellationToken == token) { throw; }
        finally { Changed?.Invoke(this, EventArgs.Empty); }
        return items switch { { Count: 0 } => 0, _ => 1 };
    }

    protected virtual int Compute(int a, int b) => a > b ? a - b : b - a;
    public static bool operator ==(Base<T> l, Base<T> r) => ReferenceEquals(l, r);
    public static bool operator !=(Base<T> l, Base<T> r) => !(l == r);
}

public enum Kind { A = 1, B, C }
"#;

/// A C# provider that runs the framework-dependent worker through the `dotnet` host.
pub fn provider(worker_dll: &Path) -> OutOfProcessProvider {
    provider_with_command(vec!["dotnet".into(), worker_dll.into()])
}

/// A C# provider whose worker is started with `command` (tests use a host that cannot run).
pub fn provider_with_command(command: Vec<OsString>) -> OutOfProcessProvider {
    let mut config = WorkerConfig::new(command, "");
    config.grammar = Some(grammars::CSHARP);
    config.pool_size = pool_size();
    // The worker is a batch process: start counting calls at once, so that its hot methods
    // reach optimized code in the first second instead of after the default 100 ms of quiet.
    config.warmup_source = WARMUP_SOURCE.into();
    config.env = vec![("DOTNET_TC_CallCountingDelayMs".into(), "0".into())];
    OutOfProcessProvider::new(
        grammars::CSHARP,
        vec![FileMatcher::extension("cs"), FileMatcher::extension("csx")],
        config,
    )
    .with_crate_name(env!("CARGO_PKG_NAME"))
    .with_format(Box::new(CSharpFormatter), option_schema())
    .with_rules(rules::RULES, rules::BODY_KINDS)
    .with_families(rules::FAMILIES)
    .with_pattern_rules(provider_api::pattern_files!())
    .with_summaries(BehaviorSummaries::new(DOES_NOT_RETURN, &[]))
}

/// Methods of the .NET standard library that never return normally and carry no
/// `[DoesNotReturn]` attribute the worker could see (qualified names; every overload).
const DOES_NOT_RETURN: &[&str] = &["System.Environment.Exit", "System.Environment.FailFast"];
