//! CommonMark 0.31.2 spec examples (`tests/conformance/fetch-markdown-corpus.sh`): per example,
//! does our tree print back to the input without parse errors, and does its block and inline
//! structure equal pulldown-cmark's. Run: `cargo test -p lang-markdown --test integration -- --ignored --nocapture commonmark::`.

use core_syntax::SyntaxNode;
use provider_api::{LanguageProvider, SourceFile};
use pulldown_cmark::{Event, Parser, Tag};
use rowan::{NodeOrToken, WalkEvent};
use std::collections::BTreeMap;

/// One letter per element that matters to the structure; paragraphs inside list items are left
/// out on both sides (a tight item has none).
fn ours(tree: &SyntaxNode, g: &core_syntax::Grammar) -> String {
    let mut out = String::new();
    let (mut items, mut inlines) = (0, 0);
    let mut stack: Vec<String> = Vec::new();
    for ev in tree.preorder_with_tokens() {
        match ev {
            WalkEvent::Enter(NodeOrToken::Node(n)) => {
                let name = g.name(n.kind()).to_string();
                let parent = n.parent().map(|p| g.name(p.kind()).to_string());
                match name.as_str() {
                    "Inline" if parent.as_deref() != Some("Heading") && items == 0 => out.push('P'),
                    "Inline" => inlines += 1,
                    "Heading" => out.push('H'),
                    "FencedCode" => out.push('C'),
                    "BlockQuote" => out.push('Q'),
                    "List" => {
                        let ordered = n
                            .descendants_with_tokens()
                            .filter_map(|e| e.into_token())
                            .find(|t| g.name(t.kind()) == "ListMarker")
                            .is_some_and(|t| {
                                t.text().trim().starts_with(|c: char| c.is_ascii_digit())
                            });
                        out.push_str(if ordered { "Lo" } else { "Lu" });
                    }
                    "ListItem" => {
                        items += 1;
                        out.push('I');
                    }
                    "Emphasis" => out.push('E'),
                    "Strong" => out.push('S'),
                    "Link" => out.push('A'),
                    "Image" => out.push('M'),
                    _ => {}
                }
                if name == "Inline" {
                    inlines += 0;
                }
                stack.push(name);
            }
            WalkEvent::Leave(NodeOrToken::Node(n)) => {
                if g.name(n.kind()) == "ListItem" {
                    items -= 1;
                }
                stack.pop();
            }
            WalkEvent::Enter(NodeOrToken::Token(t)) => {
                let in_inline = stack.iter().any(|s| s == "Inline" || s == "Cell");
                match g.name(t.kind()) {
                    "CodeText" | "IndentedCode" => {
                        if g.name(t.kind()) != "CodeText" {
                            out.push('C')
                        }
                    }
                    "HtmlBlock" => out.push('X'),
                    "HtmlComment" => out.push(if in_inline { 'x' } else { 'X' }),
                    "ThematicBreak" => out.push('R'),
                    "CodeSpan" => out.push('c'),
                    "HtmlInline" => out.push('x'),
                    "HardBreak" => out.push('B'),
                    "Autolink" => out.push('A'),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    let _ = inlines;
    out
}

fn theirs(md: &str) -> String {
    let mut out = String::new();
    let (mut items, mut in_html) = (0, 0);
    for ev in Parser::new(md) {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Paragraph if items == 0 => out.push('P'),
                Tag::Heading { .. } => out.push('H'),
                Tag::BlockQuote(_) => out.push('Q'),
                Tag::List(Some(_)) => out.push_str("Lo"),
                Tag::List(None) => out.push_str("Lu"),
                Tag::Item => {
                    items += 1;
                    out.push('I')
                }
                Tag::CodeBlock(_) => out.push('C'),
                Tag::HtmlBlock => {
                    in_html += 1;
                    out.push('X')
                }
                Tag::Emphasis => out.push('E'),
                Tag::Strong => out.push('S'),
                Tag::Link { .. } => out.push('A'),
                Tag::Image { .. } => out.push('M'),
                _ => {}
            },
            Event::End(pulldown_cmark::TagEnd::Item) => items -= 1,
            Event::End(pulldown_cmark::TagEnd::HtmlBlock) => in_html -= 1,
            Event::Code(_) => out.push('c'),
            Event::InlineHtml(_) => out.push('x'),
            Event::Rule => out.push('R'),
            Event::HardBreak => out.push('B'),
            _ => {}
        }
    }
    let _ = in_html;
    out
}

#[test]
#[ignore = "needs tests/conformance/fetch-markdown-corpus.sh"]
fn commonmark_spec_examples() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/conformance/corpus/commonmark/spec.json"
    );
    let spec: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).expect("run fetch-markdown-corpus.sh"))
            .unwrap();
    let p = lang_markdown::provider();
    let g = p.grammar();
    let (mut total, mut clean, mut lossless, mut same) = (0, 0, 0, 0);
    let mut failing: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for ex in &spec {
        let md = ex["markdown"].as_str().unwrap();
        let n = ex["example"].as_u64().unwrap();
        let r = p.parse(&SourceFile::new("x.md", 0, md)).unwrap();
        total += 1;
        clean += r.errors.is_empty() as u32;
        lossless += (r.tree.text() == md) as u32;
        let (a, b) = (ours(&r.tree, g), theirs(md));
        if a == b {
            same += 1;
        } else {
            failing
                .entry(ex["section"].as_str().unwrap().to_string())
                .or_default()
                .push(n);
            if std::env::var("MD_VERBOSE").is_ok() {
                println!("#{n} {md:?}\n  ours   {a}\n  theirs {b}");
            }
        }
    }
    println!(
        "{total} examples: {clean} without parse errors, {lossless} lossless, {same} with the structure of pulldown-cmark"
    );
    for (section, ids) in failing {
        println!("  {section}: {} differ ({ids:?})", ids.len());
    }
    assert_eq!(lossless, total, "the tree must print back to the input");
}
