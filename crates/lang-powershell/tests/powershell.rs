use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_powershell::provider(), "a.ps1", input)
}

/// Every `.ps1` and `.psm1` file of the pinned corpus (`tests/conformance/fetch-powershell-corpus.sh`)
/// that tree-sitter parses formats with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-powershell --test powershell -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    check_corpus(
        lang_powershell::provider(),
        "a.ps1",
        "powershell",
        &["ps1", "psm1"],
    );
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_powershell::provider(),
        &["a/build.ps1", "x.psm1", "mod.psd1"],
        &["ps1", "a.ps1x", "a.sh"],
    );
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
