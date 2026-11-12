use core_config::OptionValue;
use core_syntax::{SyntaxNode, print, trivia};
use lang_css::{CssProvider, Dialect};
use provider_api::format_testing::{Formatting, assert_file_names, assert_formats, check_corpus};
use provider_api::{LanguageProvider, SourceFile};

const CSS: &str = "@charset \"utf-8\";\n@import url(foo.css) screen;\n:root { --x: ; --y: {a:b}; --z: 1px }\n\
a:hover, a > b[href^='x'] { color: red !important; margin: -.5em 0 +1e3px; background: url( a.png ) }\n\
@media (min-width: 40em) and (max-width:60em) { .a { b: c } }\n\
@font-face { src: url(\"a.woff2\") format('woff2'); unicode-range: U+0025-00FF, u+4?? }\n\
/* c */ .b::before { content: \"\\\"\"; width: calc(1px + (2px * 3)) }\n";

const SCSS: &str = "@use 'sass:math';\n$map: (a: 1, b: (c: 2)) !default;\n// line\n\
@mixin m($a, $b: 2) { width: math.div($a, $b); &:hover { color: red } }\n\
.a-#{$x} { @include m(1, 2); font: { family: x; size: 1px } @extend %p; }\n\
@if $a == 1 { a: b } @else if $a { c: d } @else { e: f }\n@each $k, $v in $map { .#{$k} { v: $v } }\n";

const LESS: &str = "@color: #4D926F; @detached: { background: red; };\n// line\n\
.bordered(@width: 2px; @style: solid) { border: @width @style }\n\
.a when (@mode = huge) { .bordered(4px; dashed); @r: { x: y }; @detached(); }\n\
.b:extend(.a all) { width: ~\"calc(100% - @{w})\"; @{prop}-suffix: 1; color: @@name }\n\
@import (reference) \"foo\";\n.@{name} { a: b }\n";

const CSS_OUT: &str = r#"@charset "utf-8";
@import url(foo.css) screen;
:root {
  --x: ;
  --y: {
    a: b;
  };
  --z: 1px;
}
a:hover,
a > b[href^="x"] {
  color: red !important;
  margin: -0.5em 0 +1e3px;
  background: url(a.png);
}
@media (min-width: 40em) and (max-width: 60em) {
  .a {
    b: c;
  }
}
@font-face {
  src: url("a.woff2") format("woff2");
  unicode-range: U+0025-00FF, u+4??;
}
/* c */
.b::before {
  content: '"';
  width: calc(1px + (2px * 3));
}
"#;

const SCSS_OUT: &str = r#"@use "sass:math";
$map: (
  a: 1,
  b: (
    c: 2,
  ),
) !default;
// line
@mixin m($a, $b: 2) {
  width: math.div($a, $b);
  &:hover {
    color: red;
  }
}
.a-#{$x} {
  @include m(1, 2);
  font: {
    family: x;
    size: 1px;
  }
  @extend %p;
}
@if $a == 1 {
  a: b;
} @else if $a {
  c: d;
} @else {
  e: f;
}
@each $k, $v in $map {
  .#{$k} {
    v: $v;
  }
}
"#;

const LESS_OUT: &str = r#"@color: #4d926f;
@detached: {
  background: red;
};
// line
.bordered(@width: 2px; @style: solid) {
  border: @width @style;
}
.a when (@mode = huge) {
  .bordered(4px; dashed);
  @r: {
    x: y;
  };
  @detached();
}
.b:extend(.a all) {
  width: ~"calc(100% - @{w})";
  @{prop}-suffix: 1;
  color: @@name;
}
@import (reference) "foo";
.@{name} {
  a: b;
}
"#;

fn parse(dialect: Dialect, text: &str) -> (SyntaxNode, usize, CssProvider) {
    let p = CssProvider::new(dialect);
    let parsed = p.parse(&SourceFile::new("x", 1, text)).unwrap();
    assert_eq!(print(&parsed.tree), text, "round trip");
    let g = p.grammar();
    trivia::check_attachment(&parsed.tree, |k| g.is_trivia(k)).unwrap();
    (parsed.tree, parsed.errors.len(), p)
}

/// The kinds of the statements of the stylesheet.
fn statements(dialect: Dialect, text: &str) -> (Vec<String>, usize) {
    let (tree, errors, p) = parse(dialect, text);
    let names = tree
        .children()
        .map(|n| p.grammar().name(n.kind()).to_string())
        .collect();
    (names, errors)
}

#[test]
fn samples_parse_without_errors() {
    for (dialect, text) in [
        (Dialect::Css, CSS),
        (Dialect::Scss, SCSS),
        (Dialect::Less, LESS),
    ] {
        assert_eq!(parse(dialect, text).1, 0, "{dialect:?}");
    }
}

#[test]
fn statement_kinds() {
    let (kinds, errors) = statements(
        Dialect::Less,
        "@v: 1; a:hover { b: c } @media x { } .m(); d: e; @{p}: f; --x: {a;b} ; ;",
    );
    // Bare `;` are tokens, not nodes.
    assert_eq!(
        kinds,
        [
            "AtRule",
            "Rule",
            "AtRule",
            "Statement",
            "Declaration",
            "Declaration",
            "Declaration"
        ]
    );
    assert_eq!(errors, 0);
}

#[test]
fn dialect_tokens() {
    // `//` is a comment in SCSS and Less only; plain CSS reads two delimiters.
    for (dialect, kinds) in [(Dialect::Css, 2), (Dialect::Scss, 0)] {
        let (tree, _, p) = parse(dialect, "a { b: c } // d { e }\n");
        let delims = tree
            .descendants_with_tokens()
            .filter(|e| p.grammar().name(e.kind()) == "Delim")
            .count();
        assert_eq!(delims, kinds, "{dialect:?}");
    }
}

#[test]
fn errors_stay_in_their_statement() {
    for (text, rules) in [
        ("a { b: (c; } d { e: f }", 1),
        ("a ) b { c: d } e { f: g }", 1),
        ("a { b: \"x\n c: d } e { f: g }", 2),
        ("a { b: c } } d { e: f }", 2),
        ("a { b: c", 1),
        ("a { b: c /* x", 1),
        ("a { b: [c } d { e: f }", 1),
    ] {
        let (tree, errors, p) = parse(Dialect::Css, text);
        assert!(errors > 0, "{text}");
        let count = tree
            .children()
            .filter(|n| p.grammar().name(n.kind()) == "Rule")
            .count();
        assert!(count >= rules, "{text}: {count} rules");
    }
}

#[test]
fn every_prefix_round_trips() {
    for (dialect, text) in [
        (Dialect::Css, CSS),
        (Dialect::Scss, SCSS),
        (Dialect::Less, LESS),
    ] {
        for end in (0..=text.len()).filter(|&i| text.is_char_boundary(i)) {
            parse(dialect, &text[..end]);
        }
    }
}

#[test]
fn deep_nesting_is_an_error() {
    let deep = format!("{}{}", "a{".repeat(300), "}".repeat(300));
    assert!(parse(Dialect::Css, &deep).1 > 0);
    let deep = format!("a:{}{}", "(".repeat(300), ")".repeat(300));
    assert!(parse(Dialect::Css, &format!("a {{ {deep} }}")).1 > 0);
}

/// Prettier's output for the samples.
#[test]
fn formats_like_prettier() {
    for (dialect, file, text, expected) in [
        (Dialect::Css, "a.css", CSS, CSS_OUT),
        (Dialect::Scss, "a.scss", SCSS, SCSS_OUT),
        (Dialect::Less, "a.less", LESS, LESS_OUT),
    ] {
        assert_eq!(
            assert_formats(CssProvider::new(dialect), file, text),
            expected
        );
    }
}

/// Signs are joined and split, which changes the token count: the check compares values.
#[test]
fn signs_join_and_split() {
    let out = assert_formats(
        CssProvider::new(Dialect::Css),
        "a.css",
        "a { b: func(+ 20px, + 20px) round(1.5) +2 1px -2px; c: 1px - 2px }\n",
    );
    assert_eq!(
        out,
        "a {\n  b: func(+20px, +20px) round(1.5) + 2 1px -2px;\n  c: 1px - 2px;\n}\n"
    );
}

/// Less inline JavaScript with its own `//` comments and braces is kept as written.
#[test]
fn less_inline_javascript_is_kept() {
    let text = ".m() {\n  @f: ~`(function() {\n  // a / b\n  var a = {x: 1};\n})()`;\n}\n";
    assert_eq!(
        assert_formats(CssProvider::new(Dialect::Less), "a.less", text),
        text
    );
}

/// SCSS interpolation nests inside a string: its own quotes do not end the string.
#[test]
fn scss_interpolation_nests_in_strings() {
    let text = "a {\n  b: \"x #{call(\"\")} y\";\n}\n";
    assert_eq!(
        assert_formats(CssProvider::new(Dialect::Scss), "a.scss", text),
        text
    );
}

#[test]
fn file_names() {
    assert_file_names(
        &CssProvider::new(Dialect::Css),
        &["a.css", "x/B.CSS"],
        &["a.scss", "a.less"],
    );
    assert_file_names(&CssProvider::new(Dialect::Scss), &["a.scss"], &["a.css"]);
    assert_file_names(&CssProvider::new(Dialect::Less), &["a.less"], &["a.css"]);
    assert_eq!(CssProvider::new(Dialect::Less).config_base(), Some("css"));
    assert_eq!(CssProvider::new(Dialect::Css).config_base(), None);
}

/// Real-world CSS, SCSS and Less (`bash tests/conformance/fetch-css-corpus.sh` first): every file
/// parses without errors and formats as itself.
///
/// `cargo test --profile corpus -p lang-css --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-css-corpus.sh"]
fn conformance() {
    for (dialect, file, ext) in [
        (Dialect::Css, "a.css", "css"),
        (Dialect::Scss, "a.scss", "scss"),
        (Dialect::Less, "a.less", "less"),
    ] {
        // The real projects only: Prettier's fixtures hold deliberate errors and mixed dialects.
        let mut count = 0;
        for repo in [
            "bootstrap",
            "bulma",
            "ant-design",
            "Semantic-UI",
            "normalize.css",
            "docusaurus",
        ] {
            let corpus = format!("css/{repo}");
            let (n, unparsed) = check_corpus(CssProvider::new(dialect), file, &corpus, &[ext]);
            assert!(unparsed.is_empty(), "{repo} {ext}: {unparsed:?}");
            count += n;
        }
        assert!(count > 0, "{ext}");
    }
}

fn format_with(file: &'static str, input: &str, options: &[(&'static str, OptionValue)]) -> String {
    let dialect = if file.ends_with("scss") {
        Dialect::Scss
    } else {
        Dialect::Css
    };
    let mut subject = Formatting::new(CssProvider::new(dialect), file);
    subject.options = options.to_vec();
    core_format::testing::assert_formats(&subject, input)
}

#[test]
fn rules_values_comments_and_ignore() {
    for (input, expected) in [
        (
            "a{color:red;;}\n\n\n\nb{}\n",
            "a {\n  color: red;\n}\n\nb {\n}\n",
        ),
        (
            "/* prettier-ignore */\na   {b:c}\nd{e:f}\n",
            "/* prettier-ignore */\na   {b:c}\nd {\n  e: f;\n}\n",
        ),
        (
            "a {\n  /* sudsy-ignore */\n  b:   c;\n  d:   e;\n}\n",
            "a {\n  /* sudsy-ignore */\n  b:   c;\n  d: e;\n}\n",
        ),
        (
            "a { /* c */ b: c; /* t */\n\n\n  /* d */\n\n  e: f\n}\n",
            "a {\n  /* c */\n  b: c; /* t */\n\n  /* d */\n\n  e: f;\n}\n",
        ),
        (
            "a::before,a:HOVER>b+c~d  e{margin:-.5em auto 0 0.0px;width:calc(1px+2px*3)}\n",
            "a::before,\na:hover > b + c ~ d e {\n  margin: -0.5em auto 0 0px;\n  width: calc(1px+2px * 3);\n}\n",
        ),
        (
            "a[ href = x ]:nth-child(2N+1):not(.b,.c){COLOR:#FFF!IMPORTANT}\n",
            "a[href=\"x\"]:nth-child(2n + 1):not(.b, .c) {\n  color: #fff !important;\n}\n",
        ),
        (
            "@media (min-width:100px) and (max-width:200px){a{b:c}}\n@import url('a.css') screen;\n",
            "@media (min-width: 100px) and (max-width: 200px) {\n  a {\n    b: c;\n  }\n}\n@import url(\"a.css\") screen;\n",
        ),
        (
            "a{transition:opacity .3s ease,transform .3s ease;font-family:a,b;--x:1px  2px}\n",
            "a {\n  transition:\n    opacity 0.3s ease,\n    transform 0.3s ease;\n  font-family: a, b;\n  --x: 1px 2px;\n}\n",
        ),
        (
            "a{b:c;/* x */;/* y */\n;d:e}\n/* z */;\n",
            "a {\n  b: c; /* x */\n  ; /* y */\n  d: e;\n}\n/* z */\n;\n",
        ),
        (
            "a{grid-template-areas:\"a b\"\n\"c d\";background:url(a.png)no-repeat}\n",
            "a {\n  grid-template-areas:\n    \"a b\"\n    \"c d\";\n  background: url(a.png) no-repeat;\n}\n",
        ),
    ] {
        assert_eq!(format_with("a.css", input, &[]), expected, "{input}");
    }
}

#[test]
fn options() {
    let single = [("single_quote", OptionValue::Bool(true))];
    assert_eq!(
        format_with("a.css", "a{b:\"x\";c:'y';d:\"it's\"}[a=\"b\"]{}\n", &single),
        "a {\n  b: 'x';\n  c: 'y';\n  d: \"it's\";\n}\n[a='b'] {\n}\n"
    );
    let none = [("max_blank_lines", OptionValue::Int(0))];
    assert_eq!(
        format_with("a.css", "a{}\n\n\nb{}\n", &none),
        "a {\n}\nb {\n}\n"
    );
    let tabs = [("indent_style", OptionValue::String("tab".into()))];
    assert_eq!(
        format_with("a.scss", "a{b:c}\n", &tabs),
        "a {\n\tb: c;\n}\n"
    );
}
