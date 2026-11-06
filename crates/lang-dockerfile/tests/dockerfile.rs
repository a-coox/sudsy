use core_format::DirectiveTable;
use core_format::testing::{Subject, assert_formats};
use core_syntax::{Grammar, SyntaxNode};
use core_treesitter::TreeSitterProvider;
use lang_dockerfile::option_schema;
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
            .parse(&SourceFile::new("Dockerfile", 1, text))
            .map_err(|e| e.to_string())?;
        match r.errors.first() {
            Some(e) => Err(format!("{} (byte {})", e.message, e.start)),
            None => Ok(r.tree),
        }
    }

    fn format(&self, text: &str) -> Result<String, String> {
        format_file(
            &self.0,
            &SourceFile::new("Dockerfile", 1, text),
            &option_schema().defaults(),
            &DirectiveTable::default(),
        )
        .map_err(|e| e.to_string())
    }
}

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(&S(lang_dockerfile::provider()), input)
}

/// Every file of the pinned corpus (.NET samples and images, official images, moby, buildx)
/// parses (but for the grammar gap below), formats with the same instructions and arguments (case and blanks aside: the safety
/// check) and formats idempotently.
///
/// `cargo test --profile corpus -p lang-dockerfile --test dockerfile -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/dockerfile");
    let s = S(lang_dockerfile::provider());
    let mut dirs = vec![dir];
    let (mut count, mut failures, mut unparsed) = (0, Vec::new(), Vec::new());
    while let Some(d) = dirs.pop() {
        for entry in
            std::fs::read_dir(&d).expect("run tests/conformance/fetch-dockerfile-corpus.sh")
        {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            count += 1;
            if let Err(e) = core_format::testing::check(&s, &text) {
                let e = e.lines().next().unwrap_or("");
                let list = if e.starts_with("input: syntax error") {
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
    assert!(
        failures.is_empty(),
        "{}",
        failures.join(
            "
"
        )
    );
    // The grammar fails on a continuation line that starts with `[` after another one
    // (docker-library/python, the Windows images of golang): see docs/formatting/dockerfile.md.
    assert!(
        unparsed.len() <= 40,
        "{}",
        unparsed.join(
            "
"
        )
    );
}

#[test]
fn matches_the_file_name() {
    let p = lang_dockerfile::provider();
    let m = p.file_matchers();
    for yes in [
        "a/Dockerfile",
        "Containerfile",
        "Dockerfile.dev",
        "x.dockerfile",
    ] {
        assert!(m.iter().any(|m| m.matches(&PathBuf::from(yes))), "{yes}");
    }
    for no in ["a/Dockerfiles", "docker.json", "my-Dockerfile"] {
        assert!(!m.iter().any(|m| m.matches(&PathBuf::from(no))), "{no}");
    }
}

#[test]
fn instructions_and_arguments() {
    let out = fmt(
        "# syntax=docker/dockerfile:1\n\n\n\nfrom  alpine:3.20   as build\nARG X=1  Y\nENV A=b   C=\"d e\"\nENV LEGACY  some   value\nCOPY --chown=1:1 a  b /dst/\nCOPY [\"a\",\"b\" , \"/d\"]\nCMD [ \"x\" ]\nUSER a:b\nstopsignal   SIGTERM\nHEALTHCHECK --interval=5s CMD curl -f x || exit 1\nonbuild run echo   hi\n# end",
    );
    assert_eq!(
        out,
        "# syntax=docker/dockerfile:1\n\nFROM alpine:3.20 AS build\nARG X=1 Y\nENV A=b C=\"d e\"\nENV LEGACY  some   value\nCOPY --chown=1:1 a b /dst/\nCOPY [\"a\", \"b\", \"/d\"]\nCMD [\"x\"]\nUSER a:b\nSTOPSIGNAL SIGTERM\nHEALTHCHECK --interval=5s CMD curl -f x || exit 1\nONBUILD RUN echo   hi\n# end\n"
    );
}

#[test]
fn continuations_are_reindented_outside_quotes() {
    let out = fmt(
        "RUN  apt-get update   \\\n       && apt-get install -y \\\n\n# why\n\tcurl \\\n  jq\nRUN echo \"a  \\\n   b\" \\\n  c\nCOPY a \\\n  b \\\n   /d\nENV A=1 \\\n    B=2\nRUN \\\n  x\n",
    );
    assert_eq!(
        out,
        "RUN apt-get update \\\n    && apt-get install -y \\\n\n    # why\n    curl \\\n    jq\nRUN echo \"a  \\\n   b\" \\\n    c\nCOPY a \\\n    b \\\n    /d\nENV A=1 \\\n    B=2\nRUN \\\n    x\n"
    );
}

#[test]
fn comments_between_continued_arguments_stay() {
    let out = fmt("ENV \\\n  # one\n  A=1 \\\n    # two\n  B=2\n");
    assert_eq!(out, "ENV \\\n    # one\n    A=1 \\\n    # two\n    B=2\n");
}

#[test]
fn the_escape_directive_changes_the_continuation_character() {
    let out = fmt("# escape=`\nFROM x\nRUN a  `\n  b `\n   c\nCOPY a `\n b\n");
    assert_eq!(
        out,
        "# escape=`\nFROM x\nRUN a `\n    b `\n    c\nCOPY a `\n    b\n"
    );
}

#[test]
fn heredocs_are_kept() {
    let out = fmt("RUN <<EOF\nset -e\n  echo x  \n\nEOF\nCOPY <<-A /a\n\tbody\nA\nCMD x\n");
    assert_eq!(
        out,
        "RUN <<EOF\nset -e\n  echo x  \n\nEOF\nCOPY <<-A /a\n\tbody\nA\nCMD x\n"
    );
}

#[test]
fn blank_lines_and_trailing_comments() {
    assert_eq!(
        fmt("FROM a\n\n\n\nRUN b\n\n# c\n"),
        "FROM a\n\nRUN b\n\n# c\n"
    );
    assert_eq!(fmt(""), "");
    assert_eq!(
        fmt("FROM a\r\nRUN b \\\r\n  c\r\n"),
        "FROM a\r\nRUN b \\\r\n    c\r\n"
    );
}
