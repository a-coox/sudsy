//! `cargo xtask affected [--base REV] [--all]`: what a change needs checked, from the files it
//! changes against REV (default: the merge-base with origin/main), uncommitted and untracked
//! files included. Prints one line per item, read by `scripts/gate.sh --affected` and
//! `scripts/bench.sh`:
//!   `seed <path> <crate>...`       per changed path, the crates it selects directly
//!   `full <reason>`                why everything is selected (only then)
//!   `crates <name>...`             the crates to lint and test, or `crates all`
//!   `steps <step>...`              codegen, sample-plugin, vscode, worker:<dir of workers/>
//!   `bench <folder> <crates|*>`    a `bench/inputs` folder to measure and the crates it measures
//!                                  (comma-separated: the language's and those its inputs embed)
//!
//! A crate is affected when a file in its folder changes, when it names a changed path outside its
//! folder in a string literal (`"../../tests/fixtures"`, `"workers/csharp/Sudsy.Worker"`, read at
//! build or test time) or as a path dependency, when it depends on an affected crate, and (one
//! level, its tests only) when it dev-depends on one. A changed path nothing claims selects
//! everything. The root `Cargo.lock` selects the members whose locked dependencies changed, the
//! root `Cargo.toml` those using a changed `[workspace.dependencies]` entry; any other manifest
//! change selects everything.

use anyhow::{Context, Result, bail};
use provider_api::LanguageProvider;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Never part of a change: build scratch, agent worktrees, the bench log every finished change appends to.
const IGNORED: &[&str] = &[".cargo/tmp/", ".claude/", "bench/history.jsonl"];
/// The build set-up and the scripts that select: everything runs.
const FULL: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo/config.toml",
    "xtask/",
    "scripts/gate.sh",
    "scripts/finish.sh",
    "scripts/slot-run.sh",
    "scripts/bench.sh",
    "scripts/bench-measure.sh",
];
/// Read by no test or benchmark (unless a crate claims a path in them).
const NO_CHECK: &[&str] = &[
    "docs/",
    ".github/",
    ".vscode/",
    "scripts/",
    "site/",
    // Read by scripts/parity.sh.
    "tests/parity/",
    // Corpus fetch and speed scripts: run by hand, read by ignored tests only.
    "tests/conformance/",
    "README.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "CLAUDE.md",
    "THIRD-PARTY-NOTICES.md",
    "LICENSE",
    ".gitignore",
    ".gitattributes",
];
const HARNESS: &str = "tools/perf-bench/";
const SAMPLE_PLUGIN: &str = "tests/plugins/sample/";

#[derive(Clone, Copy, PartialEq)]
pub enum Dep {
    Normal,
    Dev,
}

pub struct Crate {
    pub name: String,
    /// Relative to the repo root, `/`-separated, no trailing `/`.
    pub dir: String,
    pub deps: Vec<(String, Dep)>,
}

#[derive(Default)]
pub struct Workspace {
    pub crates: Vec<Crate>,
    /// (path relative to the root, crate that reads it): from string literals and path dependencies.
    pub claims: Vec<(String, String)>,
    /// The folders of `workers/`.
    pub workers: Vec<String>,
    /// (`bench/inputs` folder, comma-separated crates its measurement runs: the provider that
    /// claims its files, then the languages embedded in them; an empty name is unknown).
    pub bench: Vec<(String, String)>,
    /// (`Cargo.toml` or `Cargo.lock`, the crates its change reaches) when the change is one
    /// [`crate::manifests`] can narrow; otherwise the file selects everything.
    pub manifests: Vec<(String, Vec<String>)>,
}

#[derive(Default, PartialEq, Debug)]
pub struct Affected {
    /// Why everything is selected; `None` when the selection is partial.
    pub full: Option<String>,
    /// (changed path, the crates it selects directly: its own and those that claim it).
    pub seeds: Vec<(String, Vec<String>)>,
    pub crates: BTreeSet<String>,
    pub steps: BTreeSet<String>,
    pub bench: Vec<(String, String)>,
}

fn under(path: &str, prefix: &str) -> bool {
    match prefix.strip_suffix('/') {
        Some(dir) => path.starts_with(prefix) || path == dir,
        None => path == prefix || path.starts_with(&format!("{prefix}/")),
    }
}

pub fn select(ws: &Workspace, changed: &[String]) -> Affected {
    let changed: Vec<&String> = (changed.iter())
        .filter(|p| !IGNORED.iter().any(|i| under(p, i)))
        .collect();
    let mut out = Affected::default();
    let mut seeds = BTreeSet::new();
    let (mut inputs, mut harness) = (BTreeSet::new(), false);
    for path in &changed {
        if let Some((_, by)) = ws.manifests.iter().find(|(m, _)| m == *path) {
            seeds.extend(by.iter().cloned());
            out.seeds.push((path.to_string(), by.clone()));
            continue;
        }
        if let Some(f) = FULL.iter().find(|f| under(path, f)) {
            out.full
                .get_or_insert_with(|| format!("{path} (build set-up: {f})"));
            continue;
        }
        let mut by: Vec<String> = (ws.claims.iter())
            .filter(|(claimed, _)| under(path, claimed))
            .map(|(_, c)| c.clone())
            .collect();
        by.extend(
            (ws.crates.iter())
                .filter(|c| under(path, &c.dir))
                .max_by_key(|c| c.dir.len())
                .map(|c| c.name.clone()),
        );
        by.sort();
        by.dedup();
        let mut known = !by.is_empty();
        seeds.extend(by.iter().cloned());
        if !by.is_empty() {
            out.seeds.push((path.to_string(), by));
        }
        harness |= under(path, HARNESS);
        if let Some(rest) = path.strip_prefix("bench/inputs/") {
            inputs.insert(rest.split('/').next().unwrap_or("").to_string());
            known = true;
        }
        if under(path, "editors/vscode/") {
            out.steps.insert("vscode".into());
            known = true;
        }
        if under(path, SAMPLE_PLUGIN) {
            out.steps.insert("sample-plugin".into());
            known = true;
        }
        if let Some(w) = ws
            .workers
            .iter()
            .find(|w| under(path, &format!("workers/{w}/")))
        {
            out.steps.insert(format!("worker:{w}"));
            known = true;
        }
        if !known && !NO_CHECK.iter().any(|n| under(path, n)) {
            out.full
                .get_or_insert_with(|| format!("{path} (no crate claims it)"));
        }
    }
    if out.full.is_some() {
        out.crates = ws.crates.iter().map(|c| c.name.clone()).collect();
        out.steps = ["codegen", "sample-plugin", "vscode"]
            .map(String::from)
            .into();
        out.steps
            .extend(ws.workers.iter().map(|w| format!("worker:{w}")));
        out.bench = ws.bench.clone();
        out.bench.push(("startup".into(), "*".into()));
        return out;
    }
    // Normal and build dependencies carry a change up; a dev-dependency only into that crate's tests.
    let mut set = seeds;
    while let Some(c) = ws.crates.iter().find(|c| {
        !set.contains(&c.name)
            && c.deps
                .iter()
                .any(|(d, k)| *k == Dep::Normal && set.contains(d))
    }) {
        set.insert(c.name.clone());
    }
    out.crates = (ws.crates.iter())
        .filter(|c| set.contains(&c.name) || c.deps.iter().any(|(d, _)| set.contains(d)))
        .map(|c| c.name.clone())
        .collect();
    if !changed.is_empty() {
        out.steps.insert("codegen".into());
    }
    if out.crates.contains("plugin-sdk") {
        out.steps.insert("sample-plugin".into());
    }
    out.bench = (ws.bench.iter())
        .filter(|(f, cs)| {
            harness || inputs.contains(f) || cs.split(',').any(|c| c.is_empty() || set.contains(c))
        })
        .cloned()
        .collect();
    // Start-up builds every provider.
    if harness || set.contains("languages") {
        out.bench.push(("startup".into(), "*".into()));
    }
    out
}

pub fn print(a: &Affected) -> String {
    let mut s = String::new();
    for (path, by) in &a.seeds {
        s += &format!("seed {path} {}\n", by.join(" "));
    }
    if let Some(why) = &a.full {
        s += &format!("full {why}\ncrates all\n");
    } else {
        s += &format!(
            "crates {}\n",
            a.crates.iter().cloned().collect::<Vec<_>>().join(" ")
        );
    }
    s += &format!(
        "steps {}\n",
        a.steps.iter().cloned().collect::<Vec<_>>().join(" ")
    );
    for (folder, krate) in &a.bench {
        let unknown = krate.split(',').any(str::is_empty);
        s += &format!("bench {folder} {}\n", if unknown { "*" } else { krate });
    }
    s
}

/// `cargo xtask affected` itself: reads the change and the workspace, prints the selection.
pub fn run(root: &Path, base: Option<&str>, all: bool) -> Result<String> {
    let mut ws = load(root)?;
    let mut a = if all {
        select(&ws, &["Cargo.toml".into()])
    } else {
        let base = match base {
            Some(b) => b.to_string(),
            None => git(root, &["merge-base", "HEAD", "origin/main"])
                .or_else(|_| git(root, &["rev-parse", "HEAD^"]))?
                .trim()
                .to_string(),
        };
        let changed = changed_files(root, &base)?;
        ws.manifests = manifests(root, &base, &ws, &changed);
        select(&ws, &changed)
    };
    if all {
        a.full = Some("--all".into());
    }
    Ok(print(&a))
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    if !out.status.success() {
        bail!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8(out.stdout)?)
}

/// The crates the root manifest and lock changes reach, for the changes `crate::manifests` can narrow.
fn manifests(
    root: &Path,
    base: &str,
    ws: &Workspace,
    changed: &[String],
) -> Vec<(String, Vec<String>)> {
    let read = |p: &str| -> Option<(String, String)> {
        let old = git(root, &["show", &format!("{base}:{p}")]).ok()?;
        Some((old, std::fs::read_to_string(root.join(p)).ok()?))
    };
    let is_crate = |n: &String| ws.crates.iter().any(|c| &c.name == n);
    let mut out = Vec::new();
    for p in changed
        .iter()
        .filter(|p| *p == "Cargo.toml" || *p == "Cargo.lock")
    {
        let crates = match read(p) {
            Some((old, new)) if p == "Cargo.lock" => {
                Some(crate::manifests::lock_changes(&old, &new))
            }
            Some((old, new)) => crate::manifests::manifest_changes(&old, &new).map(|deps| {
                (ws.crates.iter())
                    .filter(|c| {
                        deps.contains(&c.name) || c.deps.iter().any(|(d, _)| deps.contains(d))
                    })
                    .map(|c| c.name.clone())
                    .collect()
            }),
            None => None,
        };
        if let Some(c) = crates {
            out.push((p.clone(), c.into_iter().filter(is_crate).collect()));
        }
    }
    out
}

fn changed_files(root: &Path, base: &str) -> Result<Vec<String>> {
    let diff = git(root, &["diff", "-z", "--name-only", base])?;
    let new = git(root, &["ls-files", "-z", "-o", "--exclude-standard"])?;
    Ok((diff.split('\0').chain(new.split('\0')))
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect())
}

fn load(root: &Path) -> Result<Workspace> {
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(root)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()?;
    if !out.status.success() {
        bail!("cargo metadata: {}", String::from_utf8_lossy(&out.stderr));
    }
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let ws_root = PathBuf::from(meta["workspace_root"].as_str().context("workspace_root")?);
    let rel = |p: &Path| -> Option<String> {
        let r = p.strip_prefix(&ws_root).ok()?.to_str()?.replace('\\', "/");
        Some(r.trim_end_matches('/').to_string())
    };
    let mut ws = Workspace::default();
    for p in meta["packages"].as_array().context("packages")? {
        let manifest = Path::new(p["manifest_path"].as_str().context("manifest_path")?);
        let dir = manifest
            .parent()
            .and_then(rel)
            .context("crate outside the workspace")?;
        let name = p["name"].as_str().context("name")?.to_string();
        let mut deps = Vec::new();
        for d in p["dependencies"].as_array().into_iter().flatten() {
            let kind = if d["kind"].as_str() == Some("dev") {
                Dep::Dev
            } else {
                Dep::Normal
            };
            deps.push((d["name"].as_str().unwrap_or("").to_string(), kind));
            // A path dependency that is no member (the sample plugin) is read as part of this crate.
            if let Some(path) = d["path"].as_str().and_then(|p| rel(Path::new(p))) {
                ws.claims.push((path, name.clone()));
            }
        }
        ws.crates.push(Crate { name, dir, deps });
    }
    let members: BTreeSet<_> = ws.crates.iter().map(|c| c.dir.clone()).collect();
    // A claim on a folder that holds crates (`"../.."` from a `src/` file names `crates/`) says
    // nothing: each crate's files are its own.
    let holds_crates = |p: &str| members.iter().any(|m| under(m, &format!("{p}/")));
    ws.claims.retain(|(p, _)| !holds_crates(p));
    // `grammars/<name>.grammar` is read by the build script of `lang-<name>` (build/lang-crate.rs).
    for c in &ws.crates {
        if let Some(lang) = c.name.strip_prefix("lang-") {
            ws.claims
                .push((format!("grammars/{lang}.grammar"), c.name.clone()));
        }
    }
    for c in &ws.crates {
        let dir = root.join(&c.dir);
        let mut found = BTreeSet::new();
        for file in rust_files(&dir)? {
            let text = std::fs::read_to_string(&file)?;
            for lit in string_literals(&text) {
                let bases = [dir.clone(), file.parent().unwrap_or(&dir).to_path_buf()];
                for path in literal_paths(root, &bases, lit) {
                    if !under(&path, &c.dir) && !path.is_empty() && !holds_crates(&path) {
                        found.insert(path);
                    }
                }
            }
        }
        ws.claims
            .extend(found.into_iter().map(|p| (p, c.name.clone())));
    }
    for e in std::fs::read_dir(root.join("workers"))? {
        let e = e?;
        if e.file_type()?.is_dir() {
            ws.workers
                .push(e.file_name().to_string_lossy().into_owned());
        }
    }
    let providers = languages::providers();
    for e in std::fs::read_dir(root.join("bench/inputs"))? {
        let folder = e?.file_name().to_string_lossy().into_owned();
        // As `perf-bench list`: `.editorconfig` is claimed by file name, the others by extension.
        let workflow = ".github/workflows/bench.yml".to_string();
        let samples = match folder.as_str() {
            "github-actions" => [workflow, String::new()],
            _ => [format!("bench.{folder}"), format!(".{folder}")],
        };
        if let Some(p) = providers.iter().find(|p| {
            (samples.iter()).any(|s| p.file_matchers().iter().any(|m| m.matches(s.as_ref())))
        }) {
            let mut crates = vec![p.crate_name()];
            crates.extend(embedded_crates(
                &providers,
                p.as_ref(),
                &root.join("bench/inputs").join(&folder),
            )?);
            ws.bench.push((folder, crates.join(",")));
        }
    }
    ws.bench.sort();
    Ok(ws)
}

/// The crates of the languages embedded in a folder's inputs (a Markdown fence, a Dockerfile `RUN`),
/// at any depth: the bench runs their parsers and rules too. Found as the engine finds them, by
/// parsing each input with its host. A worker language is skipped: it would start its worker.
fn embedded_crates(
    providers: &[Box<dyn LanguageProvider>],
    host: &dyn LanguageProvider,
    dir: &Path,
) -> Result<BTreeSet<&'static str>> {
    use provider_api::{SourceFile, embedded};
    let mut out = BTreeSet::new();
    if !host.embeddable() {
        return Ok(out);
    }
    let resolve = |name: &str| embedded::resolve(providers.iter().map(|p| p.as_ref()), name);
    for e in std::fs::read_dir(dir)? {
        let path = e?.path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(parsed) = host.parse(&SourceFile::new("bench", 1, text.clone())) else {
            continue;
        };
        // Depth-limited: code embedded in itself (a fence in a fence) must not loop.
        let mut stack = vec![(host, parsed.tree, text, 0)];
        while let Some((p, root, text, depth)) = stack.pop() {
            for region in embedded::parse_regions(p, &root, &text, &resolve) {
                out.insert(region.provider.crate_name());
                if depth < 4 {
                    stack.push((region.provider, region.root, region.code, depth + 1));
                }
            }
        }
    }
    out.remove(host.crate_name());
    Ok(out)
}

fn rust_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)? {
            let e = e?;
            let path = e.path();
            if e.file_type()?.is_dir() {
                if !(e.file_name() == "target" || e.file_name().to_string_lossy().starts_with('.'))
                {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|x| x == "rs") {
                out.push(path);
            }
        }
    }
    Ok(out)
}

/// The contents of the `"..."` literals of Rust source (escapes kept, `'"'` skipped).
fn string_literals(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let (mut out, mut i, mut start) = (Vec::new(), 0, None);
    while i < b.len() {
        match (b[i], start) {
            (b'\\', Some(_)) => i += 1,
            (b'"', Some(s)) => {
                out.push(&text[s..i]);
                start = None;
            }
            (b'"', None) => start = Some(i + 1),
            (b'\'', None) if b.get(i + 1) == Some(&b'"') && b.get(i + 2) == Some(&b'\'') => i += 2,
            _ => {}
        }
        i += 1;
    }
    out
}

/// The existing repo paths a literal names: relative to the crate or the file, or, when it holds
/// a `/`, to the root. A format string or glob counts up to its first `{`, `*` or `?`.
fn literal_paths(root: &Path, bases: &[PathBuf], lit: &str) -> Vec<String> {
    let lit = lit
        .split(['{', '*', '?'])
        .next()
        .unwrap_or("")
        .trim_end_matches('/');
    if lit.is_empty() || lit.len() > 200 || lit.contains(['\n', '\\', ':', ' ']) {
        return Vec::new();
    }
    let mut tries: Vec<PathBuf> = bases.iter().map(|b| b.join(lit)).collect();
    if lit.contains('/') && !lit.starts_with('.') {
        tries.push(root.join(lit));
    }
    (tries.iter())
        .filter_map(|p| {
            let p = normalize(p);
            let r = p.strip_prefix(root).ok()?.to_str()?.replace('\\', "/");
            p.exists().then_some(r)
        })
        .collect()
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, deps: &[(&str, Dep)]) -> Crate {
        Crate {
            name: name.into(),
            dir: format!("crates/{name}"),
            deps: deps.iter().map(|(d, k)| (d.to_string(), *k)).collect(),
        }
    }

    fn ws() -> Workspace {
        use Dep::*;
        Workspace {
            crates: vec![
                krate("core-syntax", &[]),
                krate("core-rules", &[("core-syntax", Normal), ("lang-json", Dev)]),
                krate("lang-json", &[("core-rules", Normal)]),
                krate("lang-shell", &[("core-rules", Normal)]),
                krate("lang-csharp", &[("core-rules", Normal)]),
                krate(
                    "languages",
                    &[("lang-json", Normal), ("lang-shell", Normal)],
                ),
                krate("cli", &[("languages", Normal)]),
                Crate {
                    dir: "tools/perf-bench".into(),
                    ..krate("perf-bench", &[("languages", Normal)])
                },
            ],
            claims: vec![
                ("tests/fixtures/csharp".into(), "lang-csharp".into()),
                ("workers/csharp/Sudsy.Worker".into(), "lang-csharp".into()),
            ],
            workers: vec!["csharp".into()],
            bench: vec![
                ("cs".into(), "lang-csharp".into()),
                ("json".into(), "lang-json".into()),
                ("md".into(), "lang-markdown,lang-shell".into()),
                ("sh".into(), "lang-shell".into()),
            ],
            manifests: vec![("Cargo.lock".into(), vec!["lang-shell".into()])],
        }
    }

    fn sel(paths: &[&str]) -> Affected {
        select(
            &ws(),
            &paths.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
        )
    }

    fn names(a: &Affected) -> Vec<&str> {
        a.crates.iter().map(String::as_str).collect()
    }

    #[test]
    fn language_change_selects_dependents_and_dev_dependents_one_level() {
        let a = sel(&["crates/lang-json/src/lib.rs"]);
        assert_eq!(a.full, None);
        // core-rules dev-depends on lang-json: its tests run, its dependents do not.
        assert_eq!(
            names(&a),
            ["cli", "core-rules", "lang-json", "languages", "perf-bench"]
        );
        let bench: Vec<_> = a.bench.iter().map(|(f, _)| f.as_str()).collect();
        assert_eq!(bench, ["json", "startup"]);
        assert_eq!(
            print(&a),
            "seed crates/lang-json/src/lib.rs lang-json\ncrates cli core-rules lang-json languages perf-bench\nsteps codegen\nbench json lang-json\nbench startup *\n"
        );
    }

    #[test]
    fn claimed_paths_and_workers() {
        let a = sel(&["workers/csharp/Sudsy.Worker/Format.cs"]);
        assert_eq!(names(&a), ["lang-csharp"]);
        assert!(a.steps.contains("worker:csharp"));
        assert_eq!(a.bench, [("cs".to_string(), "lang-csharp".to_string())]);
        assert_eq!(
            names(&sel(&["tests/fixtures/csharp/a/input.cs"])),
            ["lang-csharp"]
        );
    }

    #[test]
    fn docs_and_ignored_paths_select_nothing() {
        let a = sel(&["docs/roadmap.md", "bench/history.jsonl", ".cargo/tmp/x"]);
        assert!(a.crates.is_empty() && a.bench.is_empty());
        assert_eq!(sel(&["bench/history.jsonl"]), Affected::default());
    }

    #[test]
    fn unclaimed_paths_and_build_setup_select_everything() {
        for path in [
            "tests/new-thing/a.txt",
            "Cargo.toml",
            "xtask/src/affected.rs",
        ] {
            let a = sel(&[path]);
            assert!(
                a.full.as_deref().is_some_and(|w| w.starts_with(path)),
                "{path}"
            );
            assert_eq!(a.crates.len(), 8);
            assert!(a.steps.contains("worker:csharp") && a.bench.len() == 5);
        }
    }

    #[test]
    fn embedded_language_selects_its_hosts_bench() {
        let a = sel(&["crates/lang-shell/src/lib.rs"]);
        let bench: Vec<_> = a.bench.iter().map(|(f, _)| f.as_str()).collect();
        assert_eq!(bench, ["md", "sh", "startup"]);
        assert!(print(&a).contains("bench md lang-markdown,lang-shell\n"));
    }

    #[test]
    fn narrowed_lock_change_selects_its_crates() {
        let a = sel(&["Cargo.lock"]);
        assert_eq!(a.full, None);
        assert_eq!(names(&a), ["cli", "lang-shell", "languages", "perf-bench"]);
    }

    #[test]
    fn bench_input_and_harness() {
        let a = sel(&["bench/inputs/sh/x.sh"]);
        assert!(a.crates.is_empty());
        assert_eq!(a.bench, [("sh".to_string(), "lang-shell".to_string())]);
        let a = sel(&["tools/perf-bench/src/fixed.rs"]);
        assert_eq!((a.full, a.bench.len()), (None, 5));
    }

    #[test]
    fn literals() {
        let src = r#"let a = "../../tests/fixtures"; let q = '"'; let b = "x\"y"; let c = "workers/csharp";"#;
        assert_eq!(
            string_literals(src),
            ["../../tests/fixtures", "x\\\"y", "workers/csharp"]
        );
    }

    #[test]
    fn every_language_names_its_crate() {
        let root = crate::repo_root();
        for p in languages::providers() {
            let name = p.crate_name();
            assert!(
                root.join("crates").join(name).join("Cargo.toml").is_file(),
                "{} names crate {name:?}",
                p.language()
            );
        }
    }
}
