use core_format::DirectiveTable;
use core_format::testing::{Subject, assert_formats};
use core_syntax::{Grammar, SyntaxNode};
use core_treesitter::TreeSitterProvider;
use lang_editorconfig::option_schema;
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
            .parse(&SourceFile::new(".editorconfig", 1, text))
            .map_err(|e| e.to_string())?;
        match r.errors.first() {
            Some(e) => Err(format!("{} (byte {})", e.message, e.start)),
            None => Ok(r.tree),
        }
    }

    fn format(&self, text: &str) -> Result<String, String> {
        format_file(
            &self.0,
            &SourceFile::new(".editorconfig", 1, text),
            &option_schema().defaults(),
            &DirectiveTable::default(),
        )
        .map_err(|e| e.to_string())
    }
}

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(&S(lang_editorconfig::provider()), input)
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
    let s = S(lang_editorconfig::provider());
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
fn matches_the_file_name() {
    let p = lang_editorconfig::provider();
    let m = p.file_matchers();
    assert!(
        m.iter()
            .any(|m| m.matches(&PathBuf::from("a/b/.editorconfig")))
    );
    assert!(
        !m.iter()
            .any(|m| m.matches(&PathBuf::from("a/b/x.editorconfig")))
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
