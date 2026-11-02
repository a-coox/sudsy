//! Generated documents: random YAML trees rendered in random styles must format with every
//! check of the shared helper and every option combination.

mod common;

use common::{n, s, subject};
use core_config::OptionValue;
use core_format::testing::check;
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Node {
    Null,
    Plain(String),
    Single(String),
    Double(String),
    Block {
        folded: bool,
        chomp: usize,
        lines: Vec<String>,
    },
    Map(Vec<Entry>),
    Seq(Vec<Item>, bool),
    FlowSeq(Vec<Node>),
    FlowMap(Vec<(String, Node)>),
    Anchored(Box<Node>),
}

#[derive(Clone, Debug)]
struct Entry {
    comment: Option<String>,
    blank: bool,
    key: String,
    trailing: Option<String>,
    value: Node,
}

#[derive(Clone, Debug)]
struct Item {
    comment: Option<String>,
    blank: bool,
    value: Node,
}

fn word() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9]{0,6}( [a-z0-9]{1,5}){0,2}"
}

fn comment() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        3 => Just(None),
        1 => "[a-z ]{0,10}".prop_map(|t| Some(format!("# {t}"))),
    ]
}

fn scalar() -> impl Strategy<Value = Node> {
    prop_oneof![
        Just(Node::Null),
        word().prop_map(Node::Plain),
        r#"[a-z '"\\]{0,10}"#.prop_map(Node::Single),
        r#"[a-z '"\\]{0,10}"#.prop_map(Node::Double),
        (
            any::<bool>(),
            0usize..3,
            proptest::collection::vec("( {0,3}[a-z]{1,6}){1}", 1..4)
        )
            .prop_map(|(folded, chomp, lines)| Node::Block {
                folded,
                chomp,
                lines
            }),
    ]
}

fn node() -> impl Strategy<Value = Node> {
    scalar().prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(
                (
                    comment(),
                    any::<bool>(),
                    "[a-z]{1,5}",
                    comment(),
                    inner.clone()
                ),
                1..4
            )
            .prop_map(|v| Node::Map(
                v.into_iter()
                    .map(|(comment, blank, key, trailing, value)| Entry {
                        comment,
                        blank,
                        key,
                        trailing,
                        value
                    })
                    .collect()
            )),
            (
                proptest::collection::vec((comment(), any::<bool>(), inner.clone()), 1..4),
                any::<bool>()
            )
                .prop_map(|(v, flush)| Node::Seq(
                    v.into_iter()
                        .map(|(comment, blank, value)| Item {
                            comment,
                            blank,
                            value
                        })
                        .collect(),
                    flush
                )),
            proptest::collection::vec(word().prop_map(Node::Plain), 0..4).prop_map(Node::FlowSeq),
            proptest::collection::vec(("[a-z]{1,4}", word().prop_map(Node::Plain)), 0..3)
                .prop_map(Node::FlowMap),
            inner.prop_map(|n| Node::Anchored(Box::new(n))),
        ]
    })
}

/// Is the node written on the line of its indicator (as opposed to on following lines)?
fn inline(node: &Node) -> bool {
    match node {
        Node::Map(_) | Node::Seq(..) => false,
        Node::Anchored(inner) => inline(inner),
        _ => true,
    }
}

fn pad(indent: usize) -> String {
    " ".repeat(indent)
}

/// The node as text. The first line has no indentation; the others are indented to `indent`.
fn render(node: &Node, indent: usize, step: usize, out: &mut String) {
    match node {
        Node::Null => {}
        Node::Plain(w) => out.push_str(w),
        Node::Single(t) => out.push_str(&format!("'{}'", t.replace('\'', "''"))),
        Node::Double(t) => out.push_str(&format!(
            "\"{}\"",
            t.replace('\\', "\\\\").replace('"', "\\\"")
        )),
        Node::Block {
            folded,
            chomp,
            lines,
        } => {
            out.push(if *folded { '>' } else { '|' });
            out.push_str(["", "-", "+"][*chomp]);
            // The first line sets the indentation, so it must not start with a blank.
            for (i, l) in lines.iter().enumerate() {
                let l = if i == 0 { l.trim_start() } else { l.as_str() };
                out.push_str(&format!("\n{}{}", pad(indent + step), l));
            }
        }
        Node::FlowSeq(items) => {
            out.push('[');
            for (i, it) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",  ");
                }
                render(it, indent, step, out);
            }
            out.push(']');
        }
        Node::FlowMap(items) => {
            out.push('{');
            for (i, (k, v)) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&format!("{k}:  "));
                render(v, indent, step, out);
            }
            out.push('}');
        }
        Node::Anchored(inner) => {
            out.push_str("&anc");
            if inline(inner) {
                out.push(' ');
                render(inner, indent, step, out);
            } else {
                out.push('\n');
                out.push_str(&pad(indent));
                render(inner, indent, step, out);
            }
        }
        Node::Map(entries) => {
            for (i, e) in entries.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                    if e.blank {
                        out.push('\n');
                    }
                    out.push_str(&pad(indent));
                }
                if let (Some(c), true) = (&e.comment, i > 0) {
                    out.push_str(&format!("{c}\n{}", pad(indent)));
                }
                out.push_str(&format!("{}:", e.key));
                match &e.value {
                    Node::Null => {}
                    v if inline(v) => {
                        out.push(' ');
                        render(v, indent, step, out);
                    }
                    Node::Seq(_, true) => {
                        out.push('\n');
                        out.push_str(&pad(indent));
                        render(&e.value, indent, step, out);
                    }
                    v => {
                        out.push('\n');
                        out.push_str(&pad(indent + step));
                        render(v, indent + step, step, out);
                    }
                }
                if let (Some(c), true) = (
                    &e.trailing,
                    inline(&e.value) && !matches!(e.value, Node::Block { .. }),
                ) {
                    out.push_str(&format!(" {c}"));
                }
            }
        }
        Node::Seq(items, _) => {
            for (i, it) in items.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                    if it.blank {
                        out.push('\n');
                    }
                    out.push_str(&pad(indent));
                    if let Some(c) = &it.comment {
                        out.push_str(&format!("{c}\n{}", pad(indent)));
                    }
                }
                out.push('-');
                match &it.value {
                    Node::Null => {}
                    Node::Anchored(inner) if !inline(inner) => {
                        out.push_str(" &anc\n");
                        out.push_str(&pad(indent + 2));
                        render(inner, indent + 2, step, out);
                    }
                    v => {
                        out.push(' ');
                        render(v, indent + 2, step, out);
                    }
                }
            }
        }
    }
}

fn document(node: &Node, step: usize) -> String {
    let mut out = String::new();
    // A top-level block scalar or null needs a marker or parent; keep the root a collection.
    match node {
        Node::Map(_) | Node::Seq(..) => render(node, 0, step, &mut out),
        other => {
            out.push_str("key: ");
            render(other, 0, step, &mut out);
        }
    }
    out.push('\n');
    out
}

fn options() -> impl Strategy<Value = Vec<(&'static str, OptionValue)>> {
    (
        1i64..=4,
        prop_oneof![Just("preserve"), Just("indented"), Just("flush")],
        prop_oneof![Just("preserve"), Just("single"), Just("double")],
        prop_oneof![Just("preserve"), Just("always"), Just("never")],
        0i64..=2,
        prop_oneof![Just(20i64), Just(40), Just(80)],
    )
        .prop_map(|(indent, seq, quotes, start, blank, width)| {
            vec![
                ("indent_size", n(indent)),
                ("sequence_indent", s(seq)),
                ("quote_style", s(quotes)),
                ("document_start", s(start)),
                ("max_blank_lines", n(blank)),
                ("line_width", n(width)),
            ]
        })
}

proptest! {
    #[test]
    fn generated_documents_format_with_every_check(
        tree in node(),
        step in 1usize..=4,
        options in options(),
    ) {
        let text = document(&tree, step);
        let subject = subject(&options);
        if core_format::testing::Subject::parse(&subject, &text).is_err() {
            // The generator can produce a document the parser rightly refuses (for example a
            // flow scalar needing quotes); only valid documents are of interest.
            return Ok(());
        }
        if let Err(e) = check(&subject, &text) {
            prop_assert!(false, "options {:?}: {}\n--- input\n{}", options, e, text);
        }
    }
}
