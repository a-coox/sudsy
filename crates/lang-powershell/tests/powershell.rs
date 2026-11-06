use core_format::DirectiveTable;
use core_format::testing::{Subject, assert_formats};
use core_syntax::{Grammar, SyntaxNode};
use core_treesitter::TreeSitterProvider;
use lang_powershell::option_schema;
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
            .parse(&SourceFile::new("a.ps1", 1, text))
            .map_err(|e| e.to_string())?;
        match r.errors.first() {
            Some(e) => Err(format!("{} (byte {})", e.message, e.start)),
            None => Ok(r.tree),
        }
    }

    fn format(&self, text: &str) -> Result<String, String> {
        format_file(
            &self.0,
            &SourceFile::new("a.ps1", 1, text),
            &option_schema().defaults(),
            &DirectiveTable::default(),
        )
        .map_err(|e| e.to_string())
    }
}

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(&S(lang_powershell::provider()), input)
}

/// Every `.ps1` and `.psm1` file of the pinned corpus (`tests/conformance/fetch-powershell-corpus.sh`)
/// that tree-sitter parses formats with the same tokens (the safety check) and idempotently.
/// With `SUDSY_PS_OUT=<dir>` the outputs are written there, named like the corpus path.
///
/// `cargo test --profile corpus -p lang-powershell --test powershell -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/powershell");
    let s = S(lang_powershell::provider());
    let mut dirs = vec![dir];
    let (mut count, mut failures, mut unparsed) = (0, Vec::new(), Vec::new());
    while let Some(d) = dirs.pop() {
        for entry in
            std::fs::read_dir(&d).expect("run tests/conformance/fetch-powershell-corpus.sh")
        {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if !path.extension().is_some_and(|e| e == "ps1" || e == "psm1") {
                continue;
            }
            let text = text.trim_start_matches('\u{feff}');
            count += 1;
            if let Err(e) = core_format::testing::check(&s, text) {
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
    let p = lang_powershell::provider();
    let m = p.file_matchers();
    for yes in ["a/build.ps1", "x.psm1", "mod.psd1"] {
        assert!(m.iter().any(|m| m.matches(&PathBuf::from(yes))), "{yes}");
    }
    for no in ["ps1", "a.ps1x", "a.sh"] {
        assert!(!m.iter().any(|m| m.matches(&PathBuf::from(no))), "{no}");
    }
}

#[test]
fn braces_and_indentation() {
    assert_eq!(
        fmt(
            "function  Foo\n{\nparam($a,$b)\nif($a -eq   1){\nWrite-Host   \"hi\"   }\nelse\n{ bar }\nif ($x) { $y = 1\n}\n}\n"
        ),
        "function  Foo {\n    param($a, $b)\n    if ($a -eq 1) {\n        Write-Host   \"hi\"\n    }\n    else\n    { bar }\n    if ($x) {\n        $y = 1\n    }\n}\n"
    );
    assert_eq!(
        fmt("try {\n    x\n} catch {\n    y\n} finally {\n    z\n}\nif ($a) { 1 } else { 2 }\n"),
        "try {\n    x\n}\ncatch {\n    y\n}\nfinally {\n    z\n}\nif ($a) { 1 } else { 2 }\n"
    );
}

#[test]
fn operators_and_separators() {
    assert_eq!(
        fmt("$x=1+2*3\n$y=$a-eq$b\n$r=1..3\nfoo(1,2)\n$a=@(1,2 ,3)\nGet-Foo|Bar\n$i+=1\n"),
        "$x = 1 + 2 * 3\n$y = $a -eq $b\n$r = 1..3\nfoo(1, 2)\n$a = @(1, 2 , 3)\nGet-Foo | Bar\n$i += 1\n"
    );
}

#[test]
fn pipelines_continuations_and_nesting() {
    assert_eq!(
        fmt(
            "$q = Get-Foo |\nWhere-Object { $_ } |\nForEach-Object {\n$_\n}\nWrite-Host `\n\"hi\" `\n\"there\"\n$x = foo(@{\n  a = 1\n})\n"
        ),
        "$q = Get-Foo |\n    Where-Object { $_ } |\n    ForEach-Object {\n        $_\n    }\nWrite-Host `\n    \"hi\" `\n    \"there\"\n$x = foo(@{\n        a = 1\n    })\n"
    );
}

#[test]
fn hashtables_and_comments() {
    assert_eq!(
        fmt(
            "$h = @{\na = 1\n\n    longer = 2\n# c\nx = @{\nk = 1\n}\n}\nfunction Bar {\n        # in\n    Get-Foo   # t\n        # end\n}\n"
        ),
        "$h = @{\n    a      = 1\n\n    longer = 2\n    # c\n    x      = @{\n        k = 1\n    }\n}\nfunction Bar {\n    # in\n    Get-Foo   # t\n    # end\n}\n"
    );
}

#[test]
fn strings_stay() {
    let src = "$s = @\"\n  here $a\n\"@\n$t = 'a  b'\n$u = \"x $(1+  2)\"\n<#\n .SYNOPSIS\n   Help\n#>\nfoo\n";
    assert_eq!(fmt(src), src);
}
