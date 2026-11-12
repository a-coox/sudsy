use core_syntax::print;
use lang_html::generated::kind as k;
use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};
use provider_api::{LanguageProvider, SourceFile};

fn parse(src: &str) -> provider_api::ParseResult {
    lang_html::provider()
        .parse(&SourceFile::new("a.html", 1, src))
        .unwrap()
}

const PAGE: &str = "<!DOCTYPE html>\n<html lang=en>\n<head><title>A &amp; <b>x</b></title>\n\
<style>a > b { color: red }</style><script>if (a < b) {}</script></head>\n\
<body>\n<!-- c -->\n<p class=\"a\" hidden id='x'>one<br>two &copy; <![CDATA[ a ]] > b ]]></p>\n\
<textarea><b>x</b></textarea><img src=a.png><svg><path d=\"\"/></svg>\n</body>\n</html>\n";

#[test]
fn parses_and_prints_as_written() {
    let r = parse(PAGE);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(print(&r.tree), PAGE);
    assert_eq!(assert_formats(lang_html::provider(), "a.html", PAGE), PAGE);
}

/// `textarea` and `title` hold text, not markup; CDATA and the doctype body are tokens.
#[test]
fn rcdata_cdata_and_doctype() {
    let r = parse("<title><b></title><textarea><b>x</b></textarea><![CDATA[<a>]]><!doctype html>");
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    let count = |kind| {
        r.tree
            .descendants_with_tokens()
            .filter(|e| e.kind() == kind)
            .count()
    };
    assert_eq!(count(k::ELEMENT), 2);
    assert_eq!(count(k::RAW_TEXT), 2);
    assert_eq!(count(k::C_DATA), 1);
    assert_eq!(count(k::DOCTYPE_BODY), 1);
}

#[test]
fn syntax_errors_are_reported() {
    let r = parse("<div <p>");
    assert!(!r.errors.is_empty());
    assert_eq!(print(&r.tree), "<div <p>");
}

#[test]
fn file_names() {
    assert_file_names(
        &lang_html::provider(),
        &["a.html", "dir/b.htm"],
        &["a.xml", "a.vue"],
    );
}

/// Real-world HTML (`bash tests/conformance/fetch-html-corpus.sh` first): every file parses and
/// prints as written, or is reported as a syntax error. Prints the counts recorded in
/// docs/formatting/html.md.
///
/// `cargo test --profile corpus -p lang-html --test html -- --ignored --nocapture conformance`
#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-html-corpus.sh"]
fn conformance() {
    for repo in ["prettier", "bootstrap", "learning-area"] {
        println!("{repo}");
        let corpus = format!("html/{repo}");
        let (n, _) = check_corpus(lang_html::provider(), "a.html", &corpus, &["html", "htm"]);
        assert!(n > 0, "{repo}");
    }
}
