use provider_api::format_testing::{Formatting, assert_file_names, assert_formats};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_editorconfig::provider(), ".editorconfig", input)
}

/// The `.editorconfig` files of dotnet/runtime, roslyn and aspnetcore parse without errors, keep
/// the reader's sections and properties through the formatter, and format idempotently.
///
/// `cargo test --profile corpus -p lang-editorconfig --test editorconfig -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/editorconfig");
    let s = Formatting::new(lang_editorconfig::provider(), ".editorconfig");
    for entry in std::fs::read_dir(dir).expect("run tests/conformance/fetch-editorconfig-corpus.sh")
    {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let out = core_format::testing::check(&s, &text)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            core_config::outline(&text),
            core_config::outline(&out),
            "{}: sections or properties changed",
            path.display()
        );
        println!("{}: ok", path.display());
    }
}

#[test]
fn keeps_comments_at_the_end_of_the_file() {
    assert_eq!(
        fmt("[*]\na = 1\n\n\n# tail\n; last"),
        "[*]\na = 1\n\n# tail\n; last\n"
    );
    assert_eq!(fmt("# only"), "# only\n");
    assert_eq!(fmt("\n\n# only\n"), "# only\n");
    assert_eq!(fmt("[*]\n# only"), "[*]\n# only\n");
    assert_eq!(fmt("[*]\na=1\n# tail\n"), "[*]\na = 1\n# tail\n");
    assert_eq!(fmt(""), "");
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_editorconfig::provider(),
        &["a/b/.editorconfig"],
        &["a/b/x.editorconfig"],
    );
}

#[test]
fn formats_sections_pairs_and_comments() {
    let out = fmt(
        "# top\n\nroot=true  \n\n\n[*.cs]   \nindent_style=space\nINDENT_SIZE   =   4 ; not a comment\n; note\n# end\n[*.{json,yml}]\n\n\nindent_size = 2\nempty =\n[x]",
    );
    assert_eq!(
        out,
        "# top\n\nroot = true\n\n[*.cs]\nindent_style = space\nINDENT_SIZE = 4 ; not a comment\n\n; note\n# end\n[*.{json,yml}]\nindent_size = 2\nempty =\n\n[x]\n"
    );
}
