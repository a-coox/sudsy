//! A language that is not installed: its files are skipped with one line per language and a
//! distinct exit status, unless the config ignores the language.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn sudsy(dir: &Path, cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .args(args)
        .current_dir(dir)
        .env("SUDSY_CACHE_DIR", cache)
        .output()
        .unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn a_removed_language_is_skipped_until_added_or_ignored() {
    let t = tempfile::tempdir().unwrap();
    let (dir, cache) = (t.path().join("proj"), t.path().join("cache"));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.toml"), "a = 1\n").unwrap();
    fs::write(dir.join("b.toml"), "b = 1\n").unwrap();

    let out = sudsy(&dir, &cache, &["lint"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        sudsy(&dir, &cache, &["languages", "remove", "toml"])
            .status
            .success()
    );

    let out = sudsy(&dir, &cache, &["lint"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(
        stderr(&out)
            .contains("2 toml file(s) skipped: not installed; run `sudsy languages add toml`"),
        "{}",
        stderr(&out)
    );

    fs::write(dir.join("sudsy.yaml"), "languages: { toml: ignored }\n").unwrap();
    let out = sudsy(&dir, &cache, &["lint"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(!stderr(&out).contains("skipped"));

    fs::remove_file(dir.join("sudsy.yaml")).unwrap();
    assert!(
        sudsy(&dir, &cache, &["languages", "add", "toml"])
            .status
            .success()
    );
    assert_eq!(sudsy(&dir, &cache, &["lint"]).status.code(), Some(0));
}

#[test]
fn an_unknown_language_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let out = sudsy(t.path(), t.path(), &["languages", "add", "tomll"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("did you mean `toml`"),
        "{}",
        stderr(&out)
    );
}
