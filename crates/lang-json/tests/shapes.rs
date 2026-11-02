//! The shared shapes that JSON's tag defaults do not reach (block, statement sequence, verbatim),
//! layout directives, and formatting a subtree in context. They are exercised on JSON trees with
//! small test-only formatters; the rule engine that attaches directives is M2.

use core_config::OptionValue;
use core_format::shapes::{self, BlockStyle, KeyValueStyle, ListStyle};
use core_format::{
    Directive, DirectiveTable, FormatCtx, FormatError, NodeFormatter, doc, format_tree, printer,
};
use core_syntax::{SyntaxKind, SyntaxNode};
use lang_json::generated::kind as k;
use lang_json::{JsonFormatter, JsonProvider, option_schema};
use provider_api::{LanguageProvider, SourceFile};

fn parse(text: &str) -> SyntaxNode {
    let mut file = SourceFile::new("a.jsonc", 1, text);
    file.language_version = Some("jsonc".into());
    let r = JsonProvider::new().parse(&file).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    r.tree
}

fn nodes(root: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxNode> {
    root.descendants().filter(|n| n.kind() == kind).collect()
}

fn options() -> core_config::ResolvedOptions {
    let schema = option_schema();
    let mut o = schema.defaults();
    schema
        .apply(&mut o, "dialect", &OptionValue::String("jsonc".into()))
        .unwrap();
    o
}

fn run(
    formatter: &dyn NodeFormatter,
    text: &str,
    directives: impl FnOnce(&SyntaxNode, &mut DirectiveTable),
) -> String {
    let tree = parse(text);
    let mut table = DirectiveTable::default();
    directives(&tree, &mut table);
    let options = options();
    let cx = FormatCtx::new(core_syntax::grammars::json(), &options, &table, formatter);
    format_tree(&tree, &cx).unwrap()
}

/// Objects as blocks and arrays as statement sequences; everything else is JSON's own.
struct Statements {
    max_blank_lines: usize,
}

impl NodeFormatter for Statements {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<doc::Doc>, FormatError> {
        match node.kind() {
            k::OBJECT => shapes::block(
                cx,
                node,
                &BlockStyle {
                    max_blank_lines: self.max_blank_lines,
                },
            )
            .map(Some),
            k::ARRAY => {
                let items: Vec<_> = node.children().collect();
                shapes::statement_sequence(cx, &items, self.max_blank_lines).map(Some)
            }
            _ => JsonFormatter.format_node(node, cx),
        }
    }
}

fn no_directives(_: &SyntaxNode, _: &mut DirectiveTable) {}

#[test]
fn block_puts_items_on_indented_lines_and_keeps_separators() {
    let out = run(
        &Statements { max_blank_lines: 1 },
        "{\"a\":1,\n\n\n\"b\":{\"c\":2},\"d\":{}}",
        no_directives,
    );
    insta::assert_snapshot!(out, @r#"
    {
      "a": 1,

      "b": {
        "c": 2
      },
      "d": {}
    }
    "#);
}

#[test]
fn block_blank_line_limit_and_comments() {
    let input = "{\n// first\n\"a\": 1, // note\n\n\n\"b\": 2\n// end\n}";
    let out = run(&Statements { max_blank_lines: 0 }, input, no_directives);
    insta::assert_snapshot!(out, @r#"
    {
      // first
      "a": 1, // note
      "b": 2
      // end
    }
    "#);
}

#[test]
fn statement_sequence_joins_items_with_the_blank_lines_of_the_source() {
    let out = run(
        &Statements { max_blank_lines: 2 },
        "[1,\n\n\n\n2,\n3]",
        no_directives,
    );
    insta::assert_snapshot!(out, @r"
    1


    2
    3
    ");
}

#[test]
fn directive_break_all_expands_a_list_that_fits() {
    let out = run(&JsonFormatter, "[1, [2, 3]]", |t, d| {
        d.add(&nodes(t, k::ARRAY)[1], "r", Directive::BreakAll)
            .unwrap();
    });
    insta::assert_snapshot!(out, @r"
    [
      1,
      [
        2,
        3
      ]
    ]
    ");
}

#[test]
fn directive_keep_together_overrides_a_preserved_break() {
    let input = "{\n  \"a\": [\n    1, 2],\n  \"b\": 3\n}";
    let out = run(&JsonFormatter, input, |t, d| {
        for n in nodes(t, k::OBJECT).into_iter().chain(nodes(t, k::ARRAY)) {
            d.add(&n, "r", Directive::KeepTogether).unwrap();
        }
    });
    assert_eq!(out, "{ \"a\": [1, 2], \"b\": 3 }\n");
}

#[test]
fn directive_break_before_and_after_expand_the_list() {
    let out = run(&JsonFormatter, "[1, 2, 3, 4]", |t, d| {
        let items: Vec<_> = nodes(t, k::LITERAL);
        d.add(&items[1], "r", Directive::BreakBefore).unwrap();
        d.add(&items[2], "r", Directive::BreakAfter).unwrap();
    });
    insta::assert_snapshot!(out, @r"
    [
      1,
      2,
      3,
      4
    ]
    ");
}

#[test]
fn directive_blank_line_before_replaces_the_source_blank_lines() {
    let out = run(&JsonFormatter, "[\n  1,\n\n  2,\n  3,\n  4\n]", |t, d| {
        let items = nodes(t, k::LITERAL);
        d.add(&items[1], "r", Directive::BlankLineBefore(0))
            .unwrap();
        d.add(&items[3], "r", Directive::BlankLineBefore(2))
            .unwrap();
    });
    insta::assert_snapshot!(out, @r"
    [
      1,
      2,
      3,


      4
    ]
    ");
}

#[test]
fn directive_preserve_leaves_the_node_as_written() {
    let out = run(&JsonFormatter, "[1,   [2,\n  3],   4]", |t, d| {
        d.add(&nodes(t, k::ARRAY)[1], "r", Directive::Preserve)
            .unwrap();
    });
    // Text with a line break in it keeps the enclosing list expanded.
    assert_eq!(
        out,
        "[
  1,
  [2,
  3],
  4
]
"
    );
}

#[test]
fn directive_align_lines_up_key_value_separators() {
    let input = "{\n\"a\": 1,\n\"bbb\": 2,\n\"cc\": 3\n}";
    let out = run(&AlignedKeys, input, |t, d| {
        d.add(&nodes(t, k::OBJECT)[0], "r", Directive::Align)
            .unwrap();
    });
    insta::assert_snapshot!(out, @r#"
    {
      "a"  : 1,
      "bbb": 2,
      "cc" : 3
    }
    "#);
    // Only when the object is expanded.
    let flat = run(&AlignedKeys, "{\"a\": 1, \"bbb\": 2}", |t, d| {
        d.add(&nodes(t, k::OBJECT)[0], "r", Directive::Align)
            .unwrap();
    });
    assert_eq!(flat, "{ \"a\": 1, \"bbb\": 2 }\n");
}

/// Members with a space before the colon, to show that the shape's style is the provider's.
struct AlignedKeys;

impl NodeFormatter for AlignedKeys {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<doc::Doc>, FormatError> {
        if node.kind() == k::MEMBER {
            let style = KeyValueStyle::default();
            return shapes::key_value(cx, node, &style).map(Some);
        }
        JsonFormatter.format_node(node, cx)
    }
}

#[test]
fn key_value_style_controls_spacing() {
    struct Spaced;
    impl NodeFormatter for Spaced {
        fn format_node(
            &self,
            node: &SyntaxNode,
            cx: &FormatCtx<'_>,
        ) -> Result<Option<doc::Doc>, FormatError> {
            if node.kind() == k::MEMBER {
                let style = KeyValueStyle {
                    space_before_separator: true,
                    space_after_separator: false,
                };
                return shapes::key_value(cx, node, &style).map(Some);
            }
            JsonFormatter.format_node(node, cx)
        }
    }
    let out = run(&Spaced, "{\"a\":1}", no_directives);
    assert_eq!(
        out,
        "{ \"a\" :1 }
"
    );
}

#[test]
fn verbatim_keeps_a_node_and_its_inner_comments_exactly() {
    struct Raw;
    impl NodeFormatter for Raw {
        fn format_node(
            &self,
            node: &SyntaxNode,
            cx: &FormatCtx<'_>,
        ) -> Result<Option<doc::Doc>, FormatError> {
            match node.kind() {
                k::MEMBER => Ok(Some(shapes::verbatim(cx, node))),
                _ => JsonFormatter.format_node(node, cx),
            }
        }
    }
    let out = run(
        &Raw,
        "{\"a\" :  [1,\n   2 /* c */],\"b\":   2}",
        no_directives,
    );
    assert_eq!(out, "{\n  \"a\" :  [1,\n   2 /* c */],\n  \"b\":   2\n}\n");
}

#[test]
fn a_subtree_is_formatted_in_its_context() {
    let tree = parse("{\"a\": {\"b\":[1,2]}, // keep\n\"c\":3}");
    let options = options();
    let table = DirectiveTable::default();
    let cx = FormatCtx::new(
        core_syntax::grammars::json(),
        &options,
        &table,
        &JsonFormatter,
    );
    let inner = &nodes(&tree, k::OBJECT)[1];
    let mut d = cx.format(inner).unwrap();
    let out = printer::print(&mut d, &core_format::print_options(&options));
    assert_eq!(out, "{ \"b\": [1, 2] }");
}

#[test]
fn a_node_without_the_promised_structure_is_a_shape_error() {
    let tree = parse("[1]");
    let options = options();
    let table = DirectiveTable::default();
    let cx = FormatCtx::new(
        core_syntax::grammars::json(),
        &options,
        &table,
        &JsonFormatter,
    );
    let literal = &nodes(&tree, k::LITERAL)[0];
    let e = shapes::key_value(&cx, literal, &KeyValueStyle::default()).unwrap_err();
    assert!(matches!(e, FormatError::Shape(_)), "{e}");
    let e = shapes::delimited_list(&cx, literal, &ListStyle::default()).unwrap_err();
    assert!(matches!(e, FormatError::Shape(_)), "{e}");
}

#[test]
fn conflicting_directives_are_an_error() {
    let tree = parse("[1]");
    let array = &nodes(&tree, k::ARRAY)[0];
    let mut table = DirectiveTable::default();
    table.add(array, "team/a", Directive::BreakAll).unwrap();
    let e = table
        .add(array, "team/b", Directive::KeepTogether)
        .unwrap_err();
    let message = FormatError::from(e).to_string();
    assert!(
        message.contains("team/a") && message.contains("team/b"),
        "{message}"
    );
}
