//! Rules about the text and structure of documentation comments: SA1625-SA1627, SA1629, SA1651.

use super::doc_comments::{
    DOC_KINDS, Decl, Element, content, documented, element_name, first_text, is_blank, text_tokens,
    xml_text,
};
use super::kind;
use core_rules::rule;
use core_rules::{Edit, Fix, FixKind, RuleContext};
use core_syntax::{SyntaxNode, SyntaxToken, TextRange, TextSize};
use std::collections::HashSet;

const PARAMETER_NOT_USED: &str = "The parameter is not used.";

/// Elements whose text SA1627 wants: StyleCop's list.
const NEEDS_TEXT: &[&str] = &["remarks", "permission", "exception", "example"];

/// Elements SA1629 leaves alone (StyleCop's default `excludeFromPunctuationCheck`).
const NO_PERIOD: &[&str] = &["seealso"];

rule! {
    pub NoDuplicateDocText {
        id: "no-duplicate-doc-text",
        category: Style,
        default: Off,
        description: "Two elements of one documentation comment with the same text (a copied and pasted \
                  `<param>`, for example). Empty elements and \"The parameter is not used.\" are ignored. No fix.",
        example: "/// <param name=\"a\">The value.</param>\n/// <param name=\"b\">The value.</param>   // describe b\nvoid M(int a, int b) { }",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::is_element) else {
            return;
        };
        let mut seen = HashSet::new();
        for element in &d.doc.elements {
            let Some(text) = xml_text(&element.node) else {
                continue;
            };
            let text = text.trim();
            if text.is_empty() || text == PARAMETER_NOT_USED {
                continue;
            }
            if !seen.insert(text.to_string()) {
                ctx.report(element.range(), "the documentation repeats the text of another element");
            }
        }
    }
}

rule! {
    pub NoDocSlashesInPlainComment {
        id: "no-doc-slashes-in-plain-comment",
        category: Style,
        default: Off,
        fix: Safe,
        description: "A `///` comment that holds only text, no XML element: a plain comment written with \
                  documentation slashes. The fix writes `//`, unless the comment documents a declaration \
                  (it would remove documentation).",
        example: "/// helper for the loop   // write //\nint i;",
        kinds: &[kind::SINGLE_LINE_DOCUMENTATION_COMMENT_TRIVIA],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        if !node.children().all(|c| c.kind() == kind::XML_TEXT) {
            return;
        }
        let attached = documented(node, ctx, |_| true).is_some();
        for token in node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == kind::DOCUMENTATION_COMMENT_EXTERIOR_TRIVIA)
        {
            let end = token.text_range().end();
            let slashes = TextRange::new(end - TextSize::from(3), end);
            let diagnostic = ctx.report(slashes, "a plain comment should not use documentation slashes");
            if !attached {
                diagnostic.fix(Fix::new(
                    "Use //",
                    FixKind::Safe,
                    vec![Edit::replace(slashes, "//")],
                ));
            }
        }
    }
}

rule! {
    pub DocTextNotEmpty {
        id: "doc-text-not-empty",
        category: Style,
        default: Off,
        description: "A `<remarks>`, `<example>`, `<exception>` or `<permission>` element without text, \
                  written `<remarks></remarks>` or `<remarks />`. No fix.",
        example: "/// <remarks></remarks>   // write something or remove it\nclass A { }",
        kinds: &[kind::XML_ELEMENT, kind::XML_EMPTY_ELEMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let element = Element::new(node.clone());
        if NEEDS_TEXT.contains(&element.name.as_str())
            && (element.is_empty_element() || element.is_blank(false))
        {
            ctx.report(
                element.range(),
                format!("the documentation text within the '{}' tag should not be empty", element.name),
            );
        }
    }
}

rule! {
    pub NoDocPlaceholder {
        id: "no-doc-placeholder",
        category: Style,
        default: Off,
        description: "A `<placeholder>` element in a documentation comment. No fix.",
        example: "/// <placeholder>todo</placeholder>   // write the documentation\nclass A { }",
        kinds: &[kind::XML_ELEMENT, kind::XML_EMPTY_ELEMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let element = Element::new(node.clone());
        if element.name == "placeholder" {
            ctx.report(element.range(), "do not use placeholder elements");
        }
    }
}

/// A place where a period is missing: the offset the diagnostic points at, and whether the
/// fix replaces the character there (a comma or semicolon) instead of adding a period.
struct Missing {
    at: TextSize,
    replace: bool,
    /// Where the period goes when it does not replace: after the text.
    after: TextSize,
    /// Is a period after the text sensible? Not after other punctuation that ends a sentence.
    fixable: bool,
}

fn is_missing(text: &str, final_paragraph: bool) -> bool {
    !text.ends_with('.')
        && !text.ends_with(".)")
        && (final_paragraph || !text.ends_with(':'))
        && !text.ends_with("-or-")
}

fn trim_end(text: &str) -> &str {
    text.trim_end_matches([' ', '\r', '\n'])
}

fn is_inline(node: &SyntaxNode) -> bool {
    matches!(
        element_name(node).as_str(),
        "c" | "paramref" | "see" | "typeparamref"
    ) && matches!(node.kind(), kind::XML_ELEMENT | kind::XML_EMPTY_ELEMENT)
}

fn is_block(node: &SyntaxNode) -> bool {
    node.kind() == kind::XML_ELEMENT
        && matches!(
            element_name(node).as_str(),
            "code" | "list" | "note" | "para"
        )
}

/// StyleCop's `TryGetLastTextElementWithContent`, including that it looks at the first text of
/// each child.
fn last_text(node: &SyntaxNode) -> Option<SyntaxNode> {
    match node.kind() {
        kind::XML_TEXT => (!is_blank(node, false)).then(|| node.clone()),
        kind::XML_ELEMENT => {
            let children: Vec<_> = content(node).collect();
            children.iter().rev().find_map(first_text)
        }
        _ => None,
    }
}

fn token_missing(token: &SyntaxToken, final_paragraph: bool, out: &mut Vec<Missing>) -> bool {
    let text = trim_end(token.text());
    if text.is_empty() {
        return false;
    }
    if is_missing(text, final_paragraph) {
        let start = token.text_range().start() + TextSize::from(text.len() as u32);
        let entity = token.kind() == kind::XML_ENTITY_LITERAL_TOKEN;
        let back = if text.ends_with(',') || text.ends_with(';') {
            1
        } else if text.ends_with(",)") || text.ends_with(";)") {
            2
        } else {
            0
        };
        out.push(Missing {
            at: start - TextSize::from(back),
            replace: back > 0 && !entity,
            after: start,
            fixable: back > 0
                || !text.ends_with(['?', '!', ':', '"', '\'', '\u{2019}', '\u{201D}']),
        });
    }
    true
}

fn section(element: &SyntaxNode, mut final_paragraph: bool, out: &mut Vec<Missing>) {
    if NO_PERIOD.contains(&element_name(element).as_str()) {
        return;
    }
    let mut done = false;
    let children: Vec<SyntaxNode> = content(element).collect();
    for child in children.iter().rev() {
        if child.kind() == kind::XML_TEXT {
            for token in text_tokens(child).iter().rev() {
                if done {
                    break;
                }
                if token_missing(token, final_paragraph, out) {
                    done = true;
                }
            }
        } else if is_inline(child) && !done {
            let end = child.text_range().end();
            let missing = match last_text(child).and_then(|t| text_tokens(&t).pop()) {
                None => true,
                Some(token) => is_missing(trim_end(token.text()), final_paragraph),
            };
            if missing {
                out.push(Missing {
                    at: end,
                    replace: false,
                    after: end,
                    fixable: true,
                });
                done = true;
            }
        } else if child.kind() == kind::XML_ELEMENT {
            if matches!(element_name(child).as_str(), "note" | "para") {
                section(child, final_paragraph, out);
            }
            if is_block(child) {
                done = false;
                final_paragraph = false;
            }
        } else if child.kind() == kind::XML_EMPTY_ELEMENT && element_name(child) == "para" {
            done = false;
            final_paragraph = false;
        }
    }
}

rule! {
    pub DocTextEndsWithPeriod {
        id: "doc-text-ends-with-period",
        category: Style,
        default: Off,
        fix: Safe,
        description: "The last text of an element of a documentation comment does not end with a period \
                  (`.` or `.)`). Each paragraph of `<para>` and `<note>` is checked, a paragraph that ends in \
                  a colon (except the last), `-or-` and `<seealso>` are left alone, and an inline element \
                  (`<see />`, `<c>`, ...) at the end counts as not ending with a period. The fix adds the \
                  period, or replaces a trailing comma or semicolon with it.",
        example: "/// <summary>Counts the items</summary>   // Counts the items.\nint Count();",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::is_element) else {
            return;
        };
        let mut missing = Vec::new();
        for element in d.doc.elements.iter().filter(|e| !e.is_empty_element()) {
            section(&element.node, true, &mut missing);
        }
        for m in missing {
            let range = TextRange::at(m.at, TextSize::from(1));
            let edit = if m.replace {
                Edit::replace(range, ".")
            } else {
                Edit::insert(m.after, ".")
            };
            let diagnostic = ctx.report(range, "the documentation text should end with a period");
            if m.fixable {
                diagnostic.fix(Fix::new("Add a period", FixKind::Safe, vec![edit]));
            }
        }
    }
}
