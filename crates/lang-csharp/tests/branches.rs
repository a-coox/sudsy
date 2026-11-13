//! Formatting the `#if` branches that the file's symbols leave inactive (`derive_symbol_sets`,
//! `preprocessor_symbol_sets`). Every case goes through the shared checks (round trip, meaning,
//! idempotency). Comparison with CSharpier: `scripts/parity.sh format csharpier`.

use crate::common::{fmt, l, subject};
use core_config::OptionValue;
use core_format::testing::assert_formats;

const NESTED: &str = "class A {\n#if X\n  void  F( ) { int  a=1; }\n#elif Y\n  void  G( ) { int  b=1;\n#if Z\n  int   zz  =  3;\n#else\n  int   qq  =  4;\n#endif\n  }\n#else\n  void  H( ) { int  c=1; }\n#endif\n}\n";

fn on() -> (&'static str, OptionValue) {
    ("derive_symbol_sets", OptionValue::Bool(true))
}

fn sets(sets: &[&[&str]]) -> (&'static str, OptionValue) {
    (
        "preprocessor_symbol_sets",
        OptionValue::List(sets.iter().map(|s| l(s)).collect()),
    )
}

#[test]
fn inactive_branches_stay_verbatim_by_default() {
    let out = fmt(NESTED, &[]);
    assert!(out.contains("void  F( ) { int  a=1; }"), "{out}");
    assert!(out.contains("int   qq  =  4;"), "{out}");
    assert!(out.contains("void H()"), "{out}");
}

#[test]
fn derived_sets_format_every_branch() {
    let out = fmt(NESTED, &[on()]);
    insta::assert_snapshot!(out);
}

#[test]
fn explicit_sets_format_the_branches_they_activate() {
    let out = fmt(NESTED, &[sets(&[&["X"], &["Y"]])]);
    assert!(out.contains("void F()"), "{out}");
    assert!(out.contains("int qq = 4;"), "{out}");
    // Nothing activates the `#if Z` branch: it stays as written.
    assert!(out.contains("int   zz  =  3;"), "{out}");
}

#[test]
fn symbols_of_the_option_decide_the_base_parse() {
    let out = assert_formats(
        &subject(&[on()]).with_symbols(&["X"]),
        "class A {\n#if X\n  int  a=1;\n#else\n  int  b=2;\n#endif\n}\n",
    );
    assert!(
        out.contains("int a = 1;") && out.contains("int b = 2;"),
        "{out}"
    );
}

#[test]
fn a_branch_that_does_not_parse_alone_stays_verbatim() {
    let input = "class A {
#if B
  int  x = ;
#endif
#if C
  int  y=1;
#endif
}
";
    let out = fmt(input, &[on()]);
    assert!(out.contains("int  x = ;"), "{out}");
    assert!(out.contains("int y = 1;"), "{out}");
}

#[test]
fn a_line_inside_a_string_that_looks_like_a_directive_stops_the_merge() {
    let input = "class A {
  string  s = @\"
#if X
\";
#if Y
  int  b=1;
#endif
}
";
    let out = fmt(input, &[on()]);
    assert!(out.contains("int  b=1;"), "{out}");
}
