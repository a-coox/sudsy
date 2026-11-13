use lang_github_actions::GithubActionsProvider;
use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

/// Every file of the pinned corpus (`tests/conformance/fetch-github-actions-corpus.sh`) that parses
/// formats with every check.
///
/// `cargo test --profile corpus -p lang-github-actions --test integration -- --ignored conformance`
#[test]
#[ignore]
fn conformance() {
    let file = ".github/workflows/a.yml";
    let (count, unparsed) = check_corpus(
        GithubActionsProvider::new(),
        file,
        "github-actions",
        &["yml", "yaml"],
    );
    println!("{count} files, {} with syntax errors", unparsed.len());
}

#[test]
fn workflows_and_actions_are_claimed_by_path() {
    assert_file_names(
        &GithubActionsProvider::new(),
        &[
            ".github/workflows/ci.yml",
            "x/.github/workflows/ci.yaml",
            "action.yml",
            "a/action.yaml",
        ],
        &[
            "ci.yml",
            "config/ci.yaml",
            "actions.yml",
            ".github/workflows/ci.json",
        ],
    );
}

#[test]
fn formats_as_yaml() {
    let out = assert_formats(
        GithubActionsProvider::new(),
        ".github/workflows/a.yml",
        "on:   push\njobs:\n  a:\n    steps:\n    - run: |\n        echo  hi\n",
    );
    assert_eq!(
        out,
        "on: push\njobs:\n  a:\n    steps:\n    - run: |\n        echo  hi\n"
    );
}
