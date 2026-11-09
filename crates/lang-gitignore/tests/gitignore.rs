use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_gitignore::provider(), ".gitignore", input)
}

/// Every `.gitignore` and `.dockerignore` of the pinned corpus
/// (`tests/conformance/fetch-gitignore-corpus.sh`) that tree-sitter parses formats with the same
/// tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-gitignore --test gitignore -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    check_corpus(lang_gitignore::provider(), ".gitignore", "gitignore", &[]);
}

#[test]
fn matches_the_file_names() {
    assert_file_names(
        &lang_gitignore::provider(),
        &[
            "a/.gitignore",
            ".dockerignore",
            "x/.npmignore",
            ".prettierignore",
        ],
        &["a/gitignore.txt", "a/exclude"],
    );
}

#[test]
fn trims_spaces_and_collapses_blank_lines() {
    let out = fmt("# top   \n\n\n\n*.log   \n!keep.log\n  \nbuild/  \n# end\n");
    assert_eq!(out, "# top\n\n*.log\n!keep.log\n\nbuild/\n# end\n");
}

#[test]
fn keeps_an_escaped_trailing_space() {
    assert_eq!(
        fmt("a\\ \nb\\ \t\nc\\   \nd\\\\ \n"),
        "a\\ \nb\\ \t\nc\\ \nd\\\\\n"
    );
}
