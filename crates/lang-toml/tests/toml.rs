use core_config::OptionValue;
use core_format::DirectiveTable;
use core_format::testing::{Subject, assert_formats};
use core_syntax::{Grammar, SyntaxNode, print, trivia};
use lang_toml::{TomlProvider, option_schema};
use provider_api::{LanguageProvider, SourceFile, format_file};
use std::path::{Path, PathBuf};

struct S {
    provider: TomlProvider,
    options: Vec<(&'static str, OptionValue)>,
}

fn subject(options: &[(&'static str, OptionValue)]) -> S {
    S {
        provider: TomlProvider::new(),
        options: options.to_vec(),
    }
}

impl Subject for S {
    fn grammar(&self) -> &Grammar {
        self.provider.grammar()
    }

    fn parse(&self, text: &str) -> Result<SyntaxNode, String> {
        let r = self
            .provider
            .parse(&SourceFile::new("a.toml", 1, text))
            .map_err(|e| e.to_string())?;
        match r.errors.first() {
            Some(e) => Err(format!("{} (byte {})", e.message, e.start)),
            None => Ok(r.tree),
        }
    }

    fn format(&self, text: &str) -> Result<String, String> {
        let schema = option_schema();
        let mut options = schema.defaults();
        for (k, v) in &self.options {
            schema.apply(&mut options, k, v).unwrap();
        }
        format_file(
            &self.provider,
            &SourceFile::new("a.toml", 1, text),
            &options,
            &DirectiveTable::default(),
        )
        .map_err(|e| e.to_string())
    }
}

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(&subject(&[]), input)
}

#[track_caller]
fn fmt_with(input: &str, options: &[(&'static str, OptionValue)]) -> String {
    assert_formats(&subject(options), input)
}

fn s(v: &str) -> OptionValue {
    OptionValue::String(v.into())
}

#[test]
fn formats_tables_pairs_arrays_inline_tables() {
    let out = fmt(
        "# top\nname='x'   # c\n\n\n[package]\nversion=\"1\"\na . b = {x=1,y=[1,2]}\n[[bin]]\nlist = [\n  1, 2,\n]\n# end\n",
    );
    assert_eq!(
        out,
        "# top\nname = 'x' # c\n\n[package]\nversion = \"1\"\na.b = { x = 1, y = [1, 2] }\n[[bin]]\nlist = [\n  1,\n  2,\n]\n# end\n"
    );
}

#[test]
fn inline_tables_never_break() {
    let long = format!("t = {{ a = \"{0}\", b = \"{0}\" }}\n", "x".repeat(60));
    assert_eq!(fmt(&long), long);
}

#[test]
fn array_options() {
    let src = "a = [\n  1, 2,\n]\nb = [1,\n  2]\n";
    assert_eq!(fmt(src), "a = [\n  1,\n  2,\n]\nb = [\n  1,\n  2,\n]\n");
    assert_eq!(
        fmt_with(src, &[("collapse", s("always"))]),
        "a = [1, 2]\nb = [1, 2]\n"
    );
    assert_eq!(
        fmt_with(
            "a = [1, 2]\n",
            &[("collapse", s("never")), ("trailing_commas", s("never"))]
        ),
        "a = [\n  1,\n  2\n]\n"
    );
}

#[test]
fn empty_and_comment_only_files() {
    assert_eq!(fmt(""), "");
    assert_eq!(fmt("# c"), "# c\n");
}

#[test]
fn crlf_is_kept() {
    assert_eq!(
        fmt("a=1\r\n[t]\r\nb='é'\r\n"),
        "a = 1\r\n[t]\r\nb = 'é'\r\n"
    );
}

#[test]
fn errors_are_lossless_and_reported() {
    let p = TomlProvider::new();
    for src in [
        "a = \n",
        "[x\nb = 1 2\n",
        "= = =\n",
        "a = [1,\n",
        "\u{e9} = 1\n",
        "a = 'é'\r\nb = ;\r\n",
    ] {
        let r = p.parse(&SourceFile::new("a.toml", 1, src)).unwrap();
        assert!(!r.errors.is_empty(), "{src:?}");
        assert_eq!(print(&r.tree), src);
        trivia::check_attachment(&r.tree, |k| p.grammar().is_trivia(k)).unwrap();
    }
}

#[test]
fn max_blank_lines_option() {
    let src = "a = 1\n\n\n\nb = 2\n";
    assert_eq!(fmt(src), "a = 1\n\nb = 2\n");
    assert_eq!(
        fmt_with(src, &[("max_blank_lines", OptionValue::Int(2))]),
        "a = 1\n\n\nb = 2\n"
    );
    assert_eq!(
        fmt_with(src, &[("max_blank_lines", OptionValue::Int(0))]),
        "a = 1\nb = 2\n"
    );
}

#[test]
fn lf_output_converts_line_breaks_inside_multiline_strings() {
    let out = fmt_with("s = \"\"\"a\r\nb\"\"\"\r\n", &[("end_of_line", s("lf"))]);
    assert_eq!(out, "s = \"\"\"a\nb\"\"\"\n");
}

#[test]
fn align_comments() {
    let src = "a = 1 # one\nlonger = 2 # two\nbb = 3\n\nc = 1 # x\n# own line\nd = 22 # y\n[t]\nx = 1 # z\nlong_key = [\n  1,\n  2,\n] # multi\ny = 2 # w\n";
    assert_eq!(fmt(src), src);
    assert_eq!(
        fmt_with(src, &[("align_comments", OptionValue::Bool(true))]),
        "a = 1      # one\nlonger = 2 # two\nbb = 3\n\nc = 1 # x\n# own line\nd = 22 # y\n[t]\nx = 1 # z\nlong_key = [\n  1,\n  2,\n] # multi\ny = 2 # w\n"
    );
}

fn error_codes(src: &str) -> Vec<String> {
    let r = TomlProvider::new()
        .parse(&SourceFile::new("a.toml", 1, src))
        .unwrap();
    r.errors.into_iter().map(|e| e.code).collect()
}

#[test]
fn semantic_errors_are_parse_errors() {
    for (src, code) in [
        ("a = 1\n'a' = 2\n", "duplicate-key"),
        ("a.b = 1\na.b.c = 2\n", "duplicate-key"),
        ("t = { k = 1, k = 2 }\n", "duplicate-key"),
        ("[a]\n[a]\n", "invalid-table"),
        ("[a.b]\n[a]\n[a]\n", "invalid-table"),
        ("[a]\nx.y = 1\n[a.x]\n", "invalid-table"),
        ("a = 1\n[a.b]\n", "invalid-table"),
        ("[a]\n[[a]]\n", "invalid-table"),
        ("[[a]]\n[a]\n", "invalid-table"),
        ("a = []\n[[a]]\n", "invalid-table"),
        ("t = {}\n[t.x]\n", "invalid-table"),
        ("d = 2023-04-31\n", "invalid-date"),
        ("d = 2023-02-29\n", "invalid-date"),
        ("d = 2023-02-30T00:00:00\n", "invalid-date"),
        ("d = 10000-01-01\n", "invalid-date"),
        ("s = \"\"\"\\q\"\"\"\n", "invalid-string"),
        ("s = \"\\uD801\"\n", "invalid-string"),
        ("s = \"\"\"a \\ b\"\"\"\n", "invalid-string"),
        ("s = '''a''''''\n", "invalid-string"),
        ("a = 1 b = 2\n", "missing-newline"),
    ] {
        assert_eq!(
            error_codes(src).first().map(String::as_str),
            Some(code),
            "{src:?}"
        );
    }
    for src in [
        "[a.b]\n[a]\nx = 1\n",
        "[a]\nb.c = 1\n[a.b.d]\n",
        "[[a]]\n[a.b]\n[[a]]\n[a.b]\n",
        "a.b = 1\na.c = 2\n",
        "t = { a.b = 1, a.c = 2 }\nu = { a.b = 1 }\n",
        "s = \"\"\"a \\\n   b\"\"\"\nd = 2024-02-29T23:59:60+01:00\nt = '''a'''''\n",
    ] {
        assert!(
            error_codes(src).is_empty(),
            "{src:?}: {:?}",
            error_codes(src)
        );
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "toml") {
            out.push(p);
        }
    }
}

/// Every file under `tests/fixtures/toml`: lossless tree, valid trivia attachment, and the shared
/// format checks.
#[test]
fn fixtures() {
    let mut files = Vec::new();
    collect(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/toml"),
        &mut files,
    );
    assert!(files.len() > 20, "fixtures missing");
    let p = TomlProvider::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let r = p.parse(&SourceFile::new("a.toml", 1, &text)).unwrap();
        assert!(r.errors.is_empty(), "{}: {:?}", f.display(), r.errors[0]);
        assert_eq!(print(&r.tree), text, "{}", f.display());
        trivia::check_attachment(&r.tree, |k| p.grammar().is_trivia(k)).unwrap();
        fmt(&text);
    }
}

/// The toml-test suite, TOML 1.0 files (`bash tests/conformance/fetch-toml-corpus.sh` first):
/// every valid file that parses must format with all checks; invalid files are counted. Prints
/// the numbers recorded in docs/formatting/toml.md.
///
/// `cargo test --profile corpus -p lang-toml --test toml -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/toml-test/tests");
    let list = std::fs::read_to_string(root.join("files-toml-1.0.0"))
        .expect("corpus missing, run tests/conformance/fetch-toml-corpus.sh");
    let p = TomlProvider::new();
    let (mut valid, mut valid_ok, mut invalid, mut invalid_rejected) = (0, 0, 0, 0);
    for rel in list.lines().filter(|l| l.ends_with(".toml")) {
        let bytes = std::fs::read(root.join(rel)).unwrap();
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let r = p.parse(&SourceFile::new("a.toml", 1, &text)).unwrap();
        assert_eq!(print(&r.tree), text, "not lossless: {rel}");
        trivia::check_attachment(&r.tree, |k| p.grammar().is_trivia(k))
            .unwrap_or_else(|e| panic!("{rel}: {e:?}"));
        if rel.starts_with("valid/") {
            valid += 1;
            if r.errors.is_empty() {
                valid_ok += 1;
                if let Err(e) = core_format::testing::check(&subject(&[]), &text) {
                    panic!("{rel}: {e}");
                }
            } else {
                println!("valid but rejected: {rel}");
            }
        } else {
            invalid += 1;
            if r.errors.is_empty() {
                println!("invalid but accepted: {rel}");
            } else {
                invalid_rejected += 1;
            }
        }
    }
    println!(
        "valid {valid}: parsed and formatted {valid_ok}; invalid {invalid}: rejected {invalid_rejected}"
    );
}
