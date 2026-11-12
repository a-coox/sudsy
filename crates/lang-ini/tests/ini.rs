use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_ini::provider(), "a.ini", input)
}

/// `cargo test --profile corpus -p lang-ini --test ini -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    check_corpus(lang_ini::provider(), "a.ini", "ini", &["ini", "cfg"]);
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_ini::provider(),
        &[
            "a/b.ini",
            "x.ini",
            "a/setup.cfg",
            "a/.gitconfig",
            "a/.gitmodules",
        ],
        &["a/x.properties"],
    );
}

#[test]
fn formats_sections_settings_and_comments() {
    let out = fmt(
        "# top\n\nroot=true  \n\n\n[core]   \nbare=false\neditor   =   vim ; kept\n; note\n[remote \"origin\"]\n\n\nurl = x\nempty =\n[x]\n",
    );
    assert_eq!(
        out,
        "# top\n\nroot = true\n\n[core]\nbare = false\neditor = vim ; kept\n\n; note\n[remote \"origin\"]\nurl = x\nempty =\n\n[x]\n"
    );
}

#[test]
fn comments_at_the_end_are_kept() {
    assert_eq!(fmt("[a]\nb=1\n\n# end\n"), "[a]\nb = 1\n\n# end\n");
    assert_eq!(fmt("[a]\n# only\n"), "[a]\n# only\n");
    assert_eq!(fmt("[a]\n# only"), "[a]\n# only\n");
    assert_eq!(fmt("[a]\nb=1\n# tail"), "[a]\nb = 1\n# tail\n");
    assert_eq!(fmt("\n\n# only\n"), "# only\n");
    assert_eq!(fmt("# only"), "# only\n");
    assert_eq!(fmt(""), "");
}

#[test]
fn a_file_without_a_final_newline_formats() {
    assert_eq!(fmt("[a]\nb=1"), "[a]\nb = 1\n");
}

#[test]
fn continuation_lines_keep_their_indentation() {
    let input = "[options]\ninstall_requires=  \n    a>=1\n  # note\n\tb == 2  \n\n    c\nname=x\n";
    assert_eq!(
        fmt(input),
        "[options]\ninstall_requires =\n    a>=1\n  # note\n\tb == 2\n\n    c\nname = x\n"
    );
}

#[test]
fn indented_settings_and_bare_keys() {
    let input = "[core]\n\tbare\n\teditor=vim\n\t# c\n\tname = x\nflag\n";
    assert_eq!(
        fmt(input),
        "[core]\nbare\neditor = vim\n# c\nname = x\nflag\n"
    );
}
