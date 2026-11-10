//! `run:` scripts are linted as Shell at their positions in the workflow, according to `shell:`.

use core_config::EditorConfig;
use core_incremental::{Database, File, lint};
use lang_github_actions::GithubActionsProvider;
use provider_api::LanguageProvider;
use std::path::PathBuf;

/// `(rule, text at the start of its range)` of the diagnostics of the workflow `text`.
fn found(path: &str, text: &str) -> Vec<(String, String)> {
    let providers: Vec<Box<dyn LanguageProvider>> = vec![
        Box::new(GithubActionsProvider::new()),
        Box::new(lang_shell::provider()),
        Box::new(lang_powershell::provider()),
    ];
    let db = Database::new(providers);
    let file = File::new(
        &db,
        PathBuf::from(path),
        1,
        text.into(),
        false,
        EditorConfig::default(),
    );
    let linted = lint(&db, file).as_ref().unwrap();
    let at = |d: &core_rules::Diagnostic| {
        text[usize::from(d.range.start())..]
            .chars()
            .take(4)
            .collect()
    };
    linted
        .diagnostics
        .iter()
        .map(|d| (d.rule.clone(), at(d)))
        .collect()
}

#[test]
fn run_is_shell_unless_the_shell_says_otherwise() {
    let job = |steps: &str, extra: &str| {
        format!("jobs:\n  a:\n    runs-on: ubuntu-latest\n{extra}    steps:\n{steps}")
    };
    let cd = "      - run: |\n          echo ${{ github.sha }}\n          cd /x; make\n";
    let expected = [("cd-without-exit".to_string(), "cd /".to_string())];
    let path = ".github/workflows/ci.yml";
    assert_eq!(found(path, &job(cd, "")), expected);
    // Indented block, an expression in the script, one-line plain scalar and `shell: bash` too.
    assert_eq!(
        found(
            path,
            &job(&format!("{cd}        shell: bash -e {{0}}\n"), "")
        ),
        expected
    );
    assert_eq!(
        found(path, &job("      - run: cd /x; make\n", "")),
        expected
    );
    // A shell the checks do not know, and a PowerShell script, are not read as Shell.
    assert!(found(path, &job(&format!("{cd}        shell: python\n"), "")).is_empty());
    let pwsh = found(path, &job(&format!("{cd}        shell: pwsh\n"), ""));
    assert!(pwsh.iter().all(|d| d.0 != "cd-without-exit"), "{pwsh:?}");
    // The job's `defaults.run.shell` applies; so does `runs-on: windows-*` (PowerShell).
    let defaults = "    defaults:\n      run:\n        shell: python\n";
    assert!(found(path, &job(cd, defaults)).is_empty());
    let windows = job(cd, "").replace("ubuntu-latest", "windows-latest");
    assert!(
        found(path, &windows)
            .iter()
            .all(|d| d.0 != "cd-without-exit")
    );
    // A composite action's steps are scripts too.
    let action = "runs:\n  using: composite\n  steps:\n    - run: cd /x; make\n      shell: bash\n";
    assert_eq!(found("action.yml", action), expected);
}
