use core_syntax::{print, trivia};
use provider_api::format_testing::{Formatting, assert_formats};
use provider_api::{LanguageProvider, SourceFile};
use std::path::{Path, PathBuf};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_xml::provider(), "a.xml", input)
}

#[test]
fn formats_elements_and_attributes() {
    let out = fmt(
        "<?xml version='1.0'?>\n<a  x='1' y=\"2\"><b/><c></c>\n\n\n<d>text  <e/> more</d><f> </f>\n<!-- c --></a>\n",
    );
    assert_eq!(
        out,
        "<?xml version='1.0'?>\n<a x=\"1\" y=\"2\">\n  <b />\n  <c></c>\n\n  <d>text  <e/> more</d>\n  <f> </f>\n  <!-- c -->\n</a>\n"
    );
}

#[test]
fn long_tags_break_their_attributes() {
    let out = fmt(&format!(
        "<a><b x=\"{0}\" y=\"{0}\"/><b x=\"{0}\" y=\"{0}\">t</b></a>",
        "v".repeat(60)
    ));
    assert!(out.contains("<b\n    x=\""), "{out}");
    assert!(out.contains("\n  />\n"), "{out}");
    assert!(out.contains("\n    >t</b>"), "{out}");
}

#[test]
fn space_preserve_keeps_content() {
    let src = "<a xml:space=\"preserve\">\n  <b/>\n   </a>\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn well_formedness_errors() {
    let p = lang_xml::provider();
    for src in [
        "<a></b>",
        "<a x=\"1\" x=\"2\"/>",
        "<a><!-- a -- b --></a>",
        "<a>]]></a>",
        "<a>",
        "<a/><b/>",
    ] {
        let r = p.parse(&SourceFile::new("a.xml", 1, src)).unwrap();
        assert!(!r.errors.is_empty(), "{src}");
        assert_eq!(print(&r.tree), src);
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// The W3C XML Conformance Test Suite (`bash tests/conformance/fetch-xml-corpus.sh` first), the
/// UTF-8 tests of XML 1.0 fifth edition: well-formed documents (`valid`, `invalid`) must parse and
/// format with all checks, `not-wf` ones should be rejected. Prints the numbers recorded in
/// docs/formatting/xml.md.
///
/// `cargo test --profile corpus -p lang-xml --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/xmlconf/xmlconf");
    let mut lists = Vec::new();
    collect(&root, &mut lists);
    let tag = regex::Regex::new(r"(?s)<TESTCASES\s([^>]*)>|</TESTCASES>|<TEST\s([^>]*)>").unwrap();
    let attr = regex::Regex::new(r#"([\w:]+)="([^"]*)""#).unwrap();
    let s = Formatting::new(lang_xml::provider(), "a.xml");
    let (mut valid, mut valid_ok, mut bad, mut bad_rejected) = (0, 0, 0, 0);
    for list in lists
        .iter()
        .filter(|l| l.extension().is_some_and(|x| x == "xml"))
    {
        let Ok(text) = std::fs::read_to_string(list) else {
            continue;
        };
        let mut bases = vec![list.parent().unwrap().to_path_buf()];
        for m in tag.captures_iter(&text) {
            if m[0].starts_with("</") {
                bases.pop();
            } else if let Some(a) = m.get(1) {
                let top = bases.last().unwrap().clone();
                let base = attr
                    .captures_iter(a.as_str())
                    .find(|c| &c[1] == "xml:base")
                    .map_or(top.clone(), |c| top.join(&c[2]));
                bases.push(base);
            } else {
                let a: std::collections::HashMap<_, _> = attr
                    .captures_iter(&m[2])
                    .map(|c| (c[1].to_string(), c[2].to_string()))
                    .collect();
                let get = |k: &str| a.get(k).map(String::as_str);
                if get("VERSION").is_some_and(|v| v != "1.0")
                    || get("EDITION").is_some_and(|e| !e.split(' ').any(|n| n == "5"))
                    || get("RECOMMENDATION").is_some_and(|r| !r.starts_with("XML1.0"))
                    || !matches!(get("TYPE"), Some("valid" | "invalid" | "not-wf"))
                {
                    continue;
                }
                let path = bases.last().unwrap().join(get("URI").unwrap());
                let Ok(doc) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let r = s
                    .provider
                    .parse(&SourceFile::new("a.xml", 1, &doc))
                    .unwrap();
                assert_eq!(print(&r.tree), doc, "not lossless: {}", path.display());
                if get("TYPE") == Some("not-wf") {
                    bad += 1;
                    if r.errors.is_empty() {
                        println!("not well-formed but accepted: {}", path.display());
                    } else {
                        bad_rejected += 1;
                    }
                } else {
                    valid += 1;
                    if !r.errors.is_empty() {
                        println!("well-formed but rejected: {}", path.display());
                    } else if let Err(e) = core_format::testing::check(&s, &doc) {
                        println!("format check failed: {}: {e}", path.display());
                    } else {
                        valid_ok += 1;
                    }
                }
            }
        }
    }
    println!(
        "well-formed {valid}: parsed and formatted {valid_ok}; not well-formed {bad}: rejected {bad_rejected}"
    );
}

/// Every file under `tests/fixtures/xml`: lossless tree, valid trivia attachment and the shared
/// format checks.
#[test]
fn fixtures() {
    let mut files = Vec::new();
    collect(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/xml"),
        &mut files,
    );
    assert!(!files.is_empty(), "fixtures missing");
    let p = lang_xml::provider();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let r = p.parse(&SourceFile::new("a.xml", 1, &text)).unwrap();
        assert!(r.errors.is_empty(), "{}: {:?}", f.display(), r.errors[0]);
        assert_eq!(print(&r.tree), text, "{}", f.display());
        trivia::check_attachment(&r.tree, |k| p.grammar().is_trivia(k)).unwrap();
        fmt(&text);
    }
}
