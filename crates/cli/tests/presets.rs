//! "Presets match their samples": every directory under
//! `tests/fixtures/presets/` is named `<language>-<preset>` and holds cases `<name>.in.<ext>` and
//! `<name>.out.<ext>`, with the sources listed in the directory's `SOURCE.md`. The `.out` file is
//! the style guide's published example (or the reference tool's output); the `.in` file is the same
//! code laid out differently. For each case, formatting the `.in` file with
//! `extends: [preset:<language>/<preset>@1]` must give the `.out` file byte for byte, and the
//! `.out` file must be unchanged by formatting (idempotency).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/presets")
}

fn csharp_worker() -> PathBuf {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workers/csharp/Sudsy.Worker");
    let status = Command::new("dotnet")
        .args(["build", "-c", "Debug", "--nologo", "-v", "q"])
        .arg(&project)
        .status()
        .expect("dotnet is installed");
    assert!(status.success(), "dotnet build failed");
    project.join("bin/Debug/net10.0/Sudsy.Worker.dll")
}

fn format(dir: &Path, worker: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
        .env("SUDSY_CSHARP_WORKER", worker)
        .current_dir(dir)
        .output()
        .unwrap()
}

/// The text between a case's name and its extension: `style-guide.in.cs` is case `style-guide`,
/// kind `in`, extension `cs`.
fn split(file: &str) -> Option<(&str, &str, &str)> {
    let (rest, ext) = file.rsplit_once('.')?;
    let (name, kind) = rest.rsplit_once('.')?;
    matches!(kind, "in" | "out").then_some((name, kind, ext))
}

#[test]
fn every_preset_reproduces_its_samples() {
    let worker = csharp_worker();
    let mut presets = 0;
    let mut cases = 0;
    let mut all_failures: Vec<String> = Vec::new();
    let mut dirs: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let dir_name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let (language, preset) = dir_name.split_once('-').unwrap();
        assert!(dir.join("SOURCE.md").is_file(), "{dir_name}: SOURCE.md");
        presets += 1;
        let mut failures = Vec::new();
        for entry in fs::read_dir(&dir).unwrap() {
            let file = entry.unwrap().file_name().to_string_lossy().into_owned();
            let Some((name, "in", ext)) = split(&file) else {
                continue;
            };
            cases += 1;
            let expected = fs::read_to_string(dir.join(format!("{name}.out.{ext}"))).unwrap();
            let work = tempfile::tempdir().unwrap();
            fs::write(
                work.path().join("sudsy.yaml"),
                format!("extends:\n  - preset:{language}/{preset}\n"),
            )
            .unwrap();
            let sample = format!("sample.{ext}");
            // The messy input, and the published output (which must not change).
            for (label, source, want) in [
                ("input", format!("{name}.in.{ext}"), &expected),
                ("output", format!("{name}.out.{ext}"), &expected),
            ] {
                fs::copy(dir.join(&source), work.path().join(&sample)).unwrap();
                let out = format(work.path(), &worker);
                let got = fs::read_to_string(work.path().join(&sample)).unwrap();
                if out.status.code() != Some(0) || got != *want {
                    failures.push(format!(
                        "{dir_name}/{name}: formatting the {label} gave:\n{got}\nexpected:\n{want}\nstderr: {}",
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
            }
        }
        all_failures.extend(failures);
    }
    assert!(
        all_failures.is_empty(),
        "{}",
        all_failures.join("\n=====\n")
    );
    assert_eq!(presets, 8, "one fixture directory per preset");
    assert!(cases >= 8);
}

/// A preset is pure data: a sudsy.yaml that is a copy of the preset file formats
/// every sample exactly like `extends: [preset:...]`.
#[test]
fn spelled_out_options_equal_the_preset() {
    let worker = csharp_worker();
    for entry in fs::read_dir(root()).unwrap() {
        let dir = entry.unwrap().path();
        let dir_name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let (language, preset) = dir_name.split_once('-').unwrap();
        // The preset file is an ordinary sudsy.yaml: copying it formats the same.
        let by_hand = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../lang-{language}/presets/{preset}.yaml")),
        )
        .unwrap();
        let configs = [
            format!("extends:\n  - preset:{language}/{preset}\n"),
            by_hand.clone(),
        ];
        for file in fs::read_dir(&dir).unwrap() {
            let file = file.unwrap().file_name().to_string_lossy().into_owned();
            let Some((name, "in", ext)) = split(&file) else {
                continue;
            };
            let results: Vec<String> = configs
                .iter()
                .map(|config| {
                    let work = tempfile::tempdir().unwrap();
                    fs::write(work.path().join("sudsy.yaml"), config).unwrap();
                    fs::copy(dir.join(&file), work.path().join(format!("sample.{ext}"))).unwrap();
                    let out = format(work.path(), &worker);
                    assert_eq!(out.status.code(), Some(0), "{dir_name}: {by_hand}");
                    fs::read_to_string(work.path().join(format!("sample.{ext}"))).unwrap()
                })
                .collect();
            assert_eq!(results[0], results[1], "{dir_name}/{name}");
        }
    }
}

/// Formatter code never names a tool: styles are options, not modes.
#[test]
fn no_tool_names_in_formatter_sources() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut stack: Vec<PathBuf> = fs::read_dir(&crates)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy().into_owned();
            n.starts_with("lang-") || n == "core-format"
        })
        .map(|p| p.join("src"))
        .collect();
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            stack.extend(fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
        } else if path.extension().is_some_and(|e| e == "rs") {
            // The provider's list of preset files (`"csharp/<name>"`, `include_str!`) names them; nothing else may.
            let text = fs::read_to_string(&path).unwrap().to_lowercase();
            let code = text.lines().filter(|l| {
                !(l.contains("include_str!") || l.trim_start().starts_with("\"csharp/"))
            });
            assert!(
                !code.collect::<String>().contains("csharpier"),
                "{}",
                path.display()
            );
        }
    }
}
