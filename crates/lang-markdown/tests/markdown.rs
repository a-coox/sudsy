use core_format::testing::Subject;
use provider_api::format_testing::Formatting;

type S = Formatting;

fn subject() -> S {
    Formatting::new(lang_markdown::provider(), "a.md")
}

#[track_caller]
fn fmt(input: &str) -> String {
    core_format::testing::assert_formats(&subject(), input)
}

#[test]
fn restyles_headings_emphasis_lists_and_fences() {
    insta::assert_snapshot!(fmt(
        "Title\n=====\n\nSub\n---\n\n## Closed ##\n\n*em* and __strong__ and snake_case and 2 * 3\n\n* a\n* b\n\n+ c\n\n3. x\n3. y\n\n~~~rs\nlet x = 1;\n~~~\n\n***\n"
    ));
}

#[test]
fn containers_keep_their_prefixes() {
    insta::assert_snapshot!(fmt(
        "> quote *a*\nlazy\n> - item\n>   more\n\n- one\n\n  para\n\n      code\n- [X] two\n\n1. a\n   ```\n   x\n   ```\n"
    ));
}

#[test]
fn aligns_tables() {
    insta::assert_snapshot!(fmt("a|b\n-|:-:\nlong cell|x\n"));
}

#[test]
fn keeps_verbatim_what_it_cannot_judge() {
    insta::assert_snapshot!(fmt(
        "<div>\n  raw   html\n</div>\n\n`code   span` and <b>x</b> and http://a/_b_c\n\n    indented   code\n\n[ref]: http://x  \"t\"\n"
    ));
}

#[test]
fn sudsy_ignore_keeps_the_next_block() {
    insta::assert_snapshot!(fmt("<!-- sudsy-ignore -->\n* a\n*   b\n\n* c\n"));
}

/// Formatting must not change what the document renders to, nor itself when repeated. Release
/// builds skip the safety check, so this is the guard against a restyle that changes meaning:
/// the repo's docs, and (after `tests/conformance/fetch-markdown-corpus.sh`) the CommonMark spec
/// examples. Run the second under the `corpus` profile:
/// `cargo test --profile corpus -p lang-markdown --test markdown -- --ignored`.
fn render(md: &str) -> String {
    let mut html = String::new();
    pulldown_cmark::html::push_html(
        &mut html,
        pulldown_cmark::Parser::new_ext(
            md,
            pulldown_cmark::Options::ENABLE_TABLES
                | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                | pulldown_cmark::Options::ENABLE_TASKLISTS,
        ),
    );
    // Runs of blanks outside code render alike (the formatter collapses them).
    let mut out = String::new();
    let (mut code, mut prev_blank) = (0, false);
    let mut rest = html.as_str();
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            code += usize::from(rest.starts_with("<code") || rest.starts_with("<pre"));
            if rest.starts_with("</code") || rest.starts_with("</pre") {
                code = code.saturating_sub(1);
            }
        }
        let blank = code == 0 && (c == ' ' || c == '\t');
        if !(blank && prev_blank) {
            out.push(c);
        }
        prev_blank = blank;
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// `None` when the output renders like the input and formatting it again changes nothing, with
/// the input's line breaks and with CRLF.
fn problem(s: &S, md: &str) -> Option<String> {
    let crlf = md.replace("\r\n", "\n").replace('\n', "\r\n");
    problem_once(s, md).or_else(|| problem_once(s, &crlf).map(|p| format!("(CRLF) {p}")))
}

fn problem_once(s: &S, md: &str) -> Option<String> {
    let out = match s.format(md) {
        Ok(o) => o,
        Err(e) => return Some(format!("refused: {e}")),
    };
    if render(md) != render(&out) {
        return Some(format!("renders differently:\n{out}"));
    }
    match s.format(&out) {
        Ok(again) if again == out => None,
        Ok(again) => Some(format!("not idempotent:\n{out}\n--- then\n{again}")),
        Err(e) => Some(format!("output refused: {e}")),
    }
}

/// The `.md` files under `dir`, recursively.
fn md_files(dir: std::path::PathBuf) -> Vec<std::path::PathBuf> {
    let (mut files, mut stack) = (Vec::new(), vec![dir]);
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                files.push(p);
            }
        }
    }
    files
}

fn assert_files_ok(files: &[std::path::PathBuf]) {
    let s = subject();
    let bad: Vec<String> = files
        .iter()
        .filter_map(|f| {
            let md = std::fs::read_to_string(f).ok()?;
            problem(&s, &md).map(|p| format!("{}: {p}", f.display()))
        })
        .collect();
    assert!(
        bad.is_empty(),
        "{}",
        bad.join(
            "
====
"
        )
    );
}

#[test]
fn repo_docs_render_the_same_and_are_idempotent() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = md_files(root.join("docs"));
    files.push(root.join("CLAUDE.md"));
    assert_files_ok(&files);
}

#[test]
#[ignore = "needs tests/conformance/fetch-markdown-corpus.sh"]
fn runtime_docs_render_the_same_and_are_idempotent() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/runtime-docs/docs");
    assert_files_ok(&md_files(dir));
}

#[test]
#[ignore = "needs tests/conformance/fetch-markdown-corpus.sh"]
fn commonmark_examples_render_the_same_and_are_idempotent() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/conformance/corpus/commonmark/spec.json"
    );
    let spec: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).expect("run fetch-markdown-corpus.sh"))
            .unwrap();
    let s = subject();
    let mut bad = Vec::new();
    for ex in &spec {
        let md = ex["markdown"].as_str().unwrap();
        // The tree's own parse is the reference for what a document means; skip the inputs it
        // reads differently from pulldown-cmark (see tests/commonmark.rs): the formatter cannot
        // be faulted for them.
        if let Some(p) = problem(&s, md) {
            bad.push(format!("#{} {md:?}: {p}", ex["example"]));
        }
    }
    println!("{} of {} examples fail", bad.len(), spec.len());
    assert!(bad.is_empty(), "{}", bad.join("\n====\n"));
}

#[test]
fn keeps_a_table_whose_row_has_more_cells_than_its_header() {
    let md = "| A | B |\n|---|---|\n| x | `a|b` |\n";
    assert_eq!(fmt(md), md);
}
