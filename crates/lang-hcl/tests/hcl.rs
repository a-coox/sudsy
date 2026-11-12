use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_hcl::provider(), "a.tf", input)
}

/// Every HCL file of the pinned corpus (`tests/conformance/fetch-hcl-corpus.sh`) parses, formats
/// with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-hcl --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (_, unparsed) = check_corpus(lang_hcl::provider(), "a.tf", "hcl", &["tf", "hcl", "nomad"]);
    assert!(unparsed.is_empty(), "valid files with syntax errors");
}

#[test]
fn indentation_follows_brackets() {
    assert_eq!(
        fmt("locals {\n    a = [1,2,\n      3]\n  b = foo(\n a,\n   b\n)\n}\n"),
        "locals {\n  a = [1, 2,\n  3]\n  b = foo(\n    a,\n    b\n  )\n}\n"
    );
}

#[test]
fn equals_and_comments_line_up_in_runs() {
    assert_eq!(
        fmt(
            "x {\n  a=1 # one\n  bbb   = 2 # two\n  # own line\n  cc = 3\n  d = {\n    e = 1\n  }\n  ff = 4\n}\n"
        ),
        "x {\n  a   = 1 # one\n  bbb = 2 # two\n  # own line\n  cc = 3\n  d = {\n    e = 1\n  }\n  ff = 4\n}\n"
    );
}

#[test]
fn spaces_between_tokens() {
    assert_eq!(
        fmt(
            "a = {x=\"y\",z = -1 }\nb = foo( a ,b...)\nc = a?b:c\nd = !x && a[ 0 ].b.*.c\ne = \"${ var.y }-${a+b}\"\nf = {for k,v in m : k=>v if v>1}\ng = {}\nh = [ ]\n"
        ),
        "a = { x = \"y\", z = -1 }\nb = foo(a, b...)\nc = a ? b : c\nd = !x && a[0].b.*.c\ne = \"${var.y}-${a + b}\"\nf = { for k, v in m : k => v if v > 1 }\ng = {}\nh = []\n"
    );
}

#[test]
fn blank_lines_heredocs_and_blocks() {
    assert_eq!(
        fmt(
            "resource   \"a\"  \"b\"{\n  x=<<-EOT\n    keep   ${ y }\n  EOT\n\n\n  z=1\n}\nempty{}\n"
        ),
        "resource \"a\" \"b\" {\n  x = <<-EOT\n    keep   ${y}\n  EOT\n\n\n  z = 1\n}\nempty {}\n"
    );
}

#[test]
fn blank_lines_at_the_end_stay() {
    assert_eq!(fmt("a = 1\n\n\n"), "a = 1\n\n\n");
    assert_eq!(fmt("a = 1"), "a = 1\n");
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_hcl::provider(),
        &["a/main.tf", "x.hcl", "a.tfvars", "job.nomad", "web.pkr.hcl"],
        &["tf", "a.tfx"],
    );
}
