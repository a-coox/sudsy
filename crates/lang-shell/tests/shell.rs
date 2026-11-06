use core_format::DirectiveTable;
use core_format::testing::{Subject, assert_formats};
use core_syntax::{Grammar, SyntaxNode};
use core_treesitter::TreeSitterProvider;
use lang_shell::option_schema;
use provider_api::{LanguageProvider, SourceFile, format_file};
use std::path::PathBuf;

struct S(TreeSitterProvider);

impl Subject for S {
    fn grammar(&self) -> &Grammar {
        self.0.grammar()
    }

    fn parse(&self, text: &str) -> Result<SyntaxNode, String> {
        let r = self
            .0
            .parse(&SourceFile::new("a.sh", 1, text))
            .map_err(|e| e.to_string())?;
        match r.errors.first() {
            Some(e) => Err(format!("{} (byte {})", e.message, e.start)),
            None => Ok(r.tree),
        }
    }

    fn format(&self, text: &str) -> Result<String, String> {
        format_file(
            &self.0,
            &SourceFile::new("a.sh", 1, text),
            &option_schema().defaults(),
            &DirectiveTable::default(),
        )
        .map_err(|e| e.to_string())
    }
}

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(&S(lang_shell::provider()), input)
}

/// Every `.sh` and `.bash` file of the pinned corpus (`tests/conformance/fetch-shell-corpus.sh`)
/// that tree-sitter parses formats with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-shell --test shell -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/shell");
    let s = S(lang_shell::provider());
    let mut dirs = vec![dir];
    let (mut count, mut failures, mut unparsed) = (0, Vec::new(), Vec::new());
    while let Some(d) = dirs.pop() {
        for entry in std::fs::read_dir(&d).expect("run tests/conformance/fetch-shell-corpus.sh") {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if !path.extension().is_some_and(|e| e == "sh" || e == "bash") {
                continue;
            }
            count += 1;
            if let Err(e) = core_format::testing::check(&s, &text) {
                let e = e.lines().next().unwrap_or("");
                let list = if e.starts_with("input: ") {
                    &mut unparsed
                } else {
                    &mut failures
                };
                list.push(format!("{}: {e}", path.display()));
            }
        }
    }
    println!(
        "{count} files, {} not parsed, {} failures",
        unparsed.len(),
        failures.len()
    );
    println!("{}", unparsed.join("\n"));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn matches_the_file_name() {
    let p = lang_shell::provider();
    let m = p.file_matchers();
    for yes in ["a/install.sh", "x.bash", ".bashrc", "home/.profile"] {
        assert!(m.iter().any(|m| m.matches(&PathBuf::from(yes))), "{yes}");
    }
    for no in ["sh", "a.shx", "profile", "bashrc.txt"] {
        assert!(!m.iter().any(|m| m.matches(&PathBuf::from(no))), "{no}");
    }
}

#[test]
fn statements_and_blank_lines() {
    assert_eq!(
        fmt(
            "#!/bin/sh\n\n\n\nset -e  # now\necho  a ;  echo b;\n\n# c\nx=1 y=2   cmd   \"$a\"  'b' \n"
        ),
        "#!/bin/sh\n\nset -e  # now\necho a\necho b\n\n# c\nx=1 y=2 cmd \"$a\" 'b'\n"
    );
    assert_eq!(fmt(""), "");
    assert_eq!(fmt("a\r\nb \\\r\n  c\r\n"), "a\r\nb \\\r\n\tc\r\n");
}

#[test]
fn compound_commands() {
    let out = fmt(
        "if [ -f a ]\nthen\n  echo x  # t\n    # before elif\nelif b; then y\nelse\n  z\n# last\nfi\nif a; then b; fi\nwhile read -r l; do echo \"$l\"; done < f\nfor i in 1 2 \\\n 3\ndo\n :\ndone\nfoo()\n{\n  { a; b; }\n  (cd x && make)\n}\n",
    );
    assert_eq!(
        out,
        "if [ -f a ]; then\n\techo x  # t\n\t# before elif\nelif b; then\n\ty\nelse\n\tz\n# last\nfi\nif a; then b; fi\nwhile read -r l; do echo \"$l\"; done <f\nfor i in 1 2 \\\n\t3; do\n\t:\ndone\nfoo() {\n\t{\n\t\ta\n\t\tb\n\t}\n\t(cd x && make)\n}\n"
    );
}

#[test]
fn case_arms() {
    let out = fmt(
        "case \"$1\" in\n  a|b)\n    echo one\n    # last\n    ;;\n  c) echo two ;;\n  *)\n    ;;\nesac\ncase $x in a) b;; esac\n",
    );
    assert_eq!(
        out,
        "case \"$1\" in\na | b)\n\techo one\n\t# last\n\t;;\nc) echo two ;;\n*)\n\t;;\nesac\ncase $x in a) b ;; esac\n"
    );
}

#[test]
fn lists_pipelines_and_continuations() {
    assert_eq!(
        fmt(
            "! cmd 2> /dev/null\na &&\n   b ||\n  c\ncmd < <(x) > f\na \\\n  && b\nx |\n y |\n z\n"
        ),
        "! cmd 2>/dev/null\na &&\n\tb ||\n\tc\ncmd < <(x) >f\na &&\n\tb\nx |\n\ty |\n\tz\n"
    );
}

#[test]
fn heredocs_and_multiline_words_are_kept() {
    let src = "cat <<EOF | grep x\n  body   \nEOF\nif a; then\n  cat <<-E >out\n\tbody\n\tE\nfi\nx=\"a  \n  b\" y=$(foo\n  bar)\necho 'a\n b'\n";
    assert_eq!(
        fmt(src),
        "cat <<EOF | grep x\n  body   \nEOF\nif a; then\n\tcat <<-E >out\n\tbody\n\tE\nfi\nx=\"a  \n  b\" y=$(foo\n  bar)\necho 'a\n b'\n"
    );
}

/// tree-sitter-bash takes a line with two assignments and the next line for one command; the
/// layout then stays as written.
#[test]
fn a_misparsed_command_is_kept() {
    assert_eq!(
        fmt("a=1\nc=1 d=2\n  foo\nfoo   bar\n"),
        "a=1\nc=1 d=2\n  foo\nfoo bar\n"
    );
}
