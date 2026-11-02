//! File discovery: which files a command applies to. Walks directories, respects `.gitignore`
//! (also outside a git checkout) and visits hidden directories such as `.vscode`, but never `.git`.
//! Ignore files, scheduling and depth modes grow here in M2.

use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("{}: no such file or directory", .0.display())]
    Missing(PathBuf),
    #[error("{0}")]
    Walk(#[from] ignore::Error),
}

/// Every file named by `paths`: files as given, directories walked. No paths means the current
/// directory. Sorted and without duplicates.
pub fn discover_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, WorkspaceError> {
    let default = [PathBuf::from(".")];
    let paths = if paths.is_empty() {
        &default[..]
    } else {
        paths
    };
    let mut out = Vec::new();
    for path in paths {
        if path.is_file() {
            out.push(path.clone());
        } else if path.is_dir() {
            walk(path, &mut out)?;
        } else {
            return Err(WorkspaceError::Missing(path.clone()));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), WorkspaceError> {
    let walker = WalkBuilder::new(dir)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walker {
        let entry = entry?;
        if entry.file_type().is_some_and(|t| t.is_file()) {
            // Walking "." yields "./a.json"; show "a.json".
            let p = entry.into_path();
            out.push(p.strip_prefix(".").map(Path::to_path_buf).unwrap_or(p));
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

    #[test]
    fn walks_respects_gitignore_and_keeps_hidden_dirs() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        for f in [
            "a.json",
            "sub/b.json",
            ".vscode/settings.json",
            "build/c.json",
            ".git/x.json",
        ] {
            touch(r, f);
        }
        fs::write(r.join(".gitignore"), "build/\n").unwrap();
        let found: Vec<String> = discover_files(&[r.to_path_buf()])
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(r)
                    .unwrap()
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            })
            .collect();
        assert_eq!(
            found,
            [
                ".gitignore",
                ".vscode/settings.json",
                "a.json",
                "sub/b.json"
            ]
        );
    }

    #[test]
    fn explicit_files_are_kept_and_missing_paths_fail() {
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), "build/c.json");
        fs::write(t.path().join(".gitignore"), "build/\n").unwrap();
        let file = t.path().join("build/c.json");
        assert_eq!(
            discover_files(&[file.clone(), file.clone()]).unwrap(),
            [file]
        );
        assert!(matches!(
            discover_files(&[t.path().join("nope")]),
            Err(WorkspaceError::Missing(_))
        ));
    }
}
