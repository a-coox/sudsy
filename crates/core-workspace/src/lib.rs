//! File discovery: which files a command applies to. Walks directories, respects `.gitignore`
//! (also outside a git checkout), leaves out what the config excludes, and visits hidden
//! directories such as `.vscode`, but never `.git`. Paths are absolute; [`display_path`] gives the
//! form printed to users. Scheduling and depth modes grow here with the rule engine.

use ignore::WalkBuilder;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("{}: no such file or directory", .0.display())]
    Missing(PathBuf),
    #[error("{0}")]
    Walk(#[from] ignore::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub type PathTest = Arc<dyn Fn(&Path) -> bool + Send + Sync>;

/// What the walk leaves out besides `.gitignore`d files.
#[derive(Clone)]
pub struct Filter {
    /// Directories and files that are never visited.
    pub exclude: PathTest,
    /// Files that are kept.
    pub include: PathTest,
}

impl Filter {
    /// Keeps everything.
    pub fn none() -> Filter {
        Filter {
            exclude: Arc::new(|_| false),
            include: Arc::new(|_| true),
        }
    }
}

/// `path` made absolute and with `.` and `..` resolved lexically (no symlinks followed).
pub fn absolute(path: &Path) -> Result<PathBuf, WorkspaceError> {
    let path = std::path::absolute(path)?;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

/// How a path is printed: relative to `base` (the working directory) with `/` separators, on every
/// platform. A path on another drive stays absolute.
pub fn display_path(path: &Path, base: &Path) -> String {
    let path: Vec<Component> = path.components().collect();
    let base: Vec<Component> = base.components().collect();
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let rooted =
        |c: &[Component]| matches!(c.first(), Some(Component::Prefix(_) | Component::RootDir));
    if common == 0 && rooted(&path) {
        return join(&path);
    }
    let mut parts: Vec<String> = base[common..].iter().map(|_| "..".to_string()).collect();
    parts.extend(
        path[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        return ".".into();
    }
    parts.join("/")
}

fn join(components: &[Component]) -> String {
    let mut out = String::new();
    for c in components {
        match c {
            Component::Prefix(p) => out.push_str(&p.as_os_str().to_string_lossy()),
            Component::RootDir => out.push('/'),
            other => {
                if !out.is_empty() && !out.ends_with('/') {
                    out.push('/');
                }
                out.push_str(&other.as_os_str().to_string_lossy());
            }
        }
    }
    out
}

/// Every file named by `paths`: files as given, directories walked (`filter` applies to what is
/// found, not to files named explicitly). No paths means the current directory. Absolute, sorted
/// and without duplicates.
pub fn discover_files(paths: &[PathBuf], filter: &Filter) -> Result<Vec<PathBuf>, WorkspaceError> {
    let default = [PathBuf::from(".")];
    let paths = if paths.is_empty() {
        &default[..]
    } else {
        paths
    };
    let mut out = Vec::new();
    for path in paths {
        let absolute = absolute(path)?;
        if absolute.is_file() {
            out.push(absolute);
        } else if absolute.is_dir() {
            walk(&absolute, filter, &mut out)?;
        } else {
            return Err(WorkspaceError::Missing(path.clone()));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk(dir: &Path, filter: &Filter, out: &mut Vec<PathBuf>) -> Result<(), WorkspaceError> {
    let exclude = filter.exclude.clone();
    let walker = WalkBuilder::new(dir)
        .hidden(false)
        .require_git(false)
        .filter_entry(move |e| e.file_name() != ".git" && !exclude(e.path()))
        .build();
    for entry in walker {
        let entry = entry?;
        if entry.file_type().is_some_and(|t| t.is_file()) && (filter.include)(entry.path()) {
            out.push(entry.into_path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "").unwrap();
    }

    fn shown(root: &Path, found: &[PathBuf]) -> Vec<String> {
        found.iter().map(|p| display_path(p, root)).collect()
    }

    #[test]
    fn walks_respects_gitignore_and_keeps_hidden_dirs() {
        let t = tempfile::tempdir().unwrap();
        let r = absolute(t.path()).unwrap();
        for f in [
            "a.json",
            "sub/b.json",
            ".vscode/settings.json",
            "build/c.json",
            ".git/x.json",
        ] {
            touch(&r, f);
        }
        fs::write(r.join(".gitignore"), "build/\n").unwrap();
        let found = discover_files(std::slice::from_ref(&r), &Filter::none()).unwrap();
        assert_eq!(
            shown(&r, &found),
            [
                ".gitignore",
                ".vscode/settings.json",
                "a.json",
                "sub/b.json"
            ]
        );
    }

    #[test]
    fn the_filter_prunes_directories_and_files_but_not_explicit_paths() {
        let t = tempfile::tempdir().unwrap();
        let r = absolute(t.path()).unwrap();
        for f in ["keep.json", "skip.txt", "gen/a.json", "src/gen.json"] {
            touch(&r, f);
        }
        let filter = Filter {
            exclude: Arc::new(|p| p.file_name().is_some_and(|n| n == "gen")),
            include: Arc::new(|p| p.extension().is_some_and(|e| e == "json")),
        };
        let found = discover_files(std::slice::from_ref(&r), &filter).unwrap();
        assert_eq!(shown(&r, &found), ["keep.json", "src/gen.json"]);
        let explicit =
            discover_files(&[r.join("gen/a.json"), r.join("skip.txt")], &filter).unwrap();
        assert_eq!(shown(&r, &explicit), ["gen/a.json", "skip.txt"]);
    }

    #[test]
    fn explicit_files_are_kept_and_missing_paths_fail() {
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), "build/c.json");
        fs::write(t.path().join(".gitignore"), "build/\n").unwrap();
        let file = absolute(&t.path().join("build/c.json")).unwrap();
        assert_eq!(
            discover_files(&[file.clone(), file.clone()], &Filter::none()).unwrap(),
            [file]
        );
        assert!(matches!(
            discover_files(&[t.path().join("nope")], &Filter::none()),
            Err(WorkspaceError::Missing(_))
        ));
    }

    #[test]
    fn paths_print_relative_with_forward_slashes() {
        let base = Path::new(if cfg!(windows) {
            "C:\\work\\repo"
        } else {
            "/work/repo"
        });
        let under = base.join("src").join("a.cs");
        assert_eq!(display_path(&under, base), "src/a.cs");
        assert_eq!(display_path(base, base), ".");
        let sibling = base.parent().unwrap().join("other").join("b.json");
        assert_eq!(display_path(&sibling, base), "../other/b.json");
        if cfg!(windows) {
            assert_eq!(
                display_path(Path::new("D:\\x\\y.json"), base),
                "D:/x/y.json"
            );
        }
    }

    #[test]
    fn absolute_resolves_dots() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(absolute(Path::new("a/./b/../c")).unwrap(), cwd.join("a/c"));
    }
}
