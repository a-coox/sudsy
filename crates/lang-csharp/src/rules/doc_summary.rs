//! The `<summary>` rules (SA1601, SA1604-SA1608). A partial type or method is checked part by
//! part: StyleCop accepts one documented part, which needs the whole project.

use super::doc_comments::{
    DOC_KINDS, Decl, Declaration, Documented, Element, doc_of, documented, normalized_text,
};
use super::kind;
use core_rules::RuleContext;
use core_rules::rule;
use core_syntax::SyntaxNode;

const DEFAULT_SUMMARY: &str = "Summary description for";

/// The element the partial-element rules read: `<summary>`, else `<content>`.
fn summary_or_content(d: &Documented) -> Option<&Element> {
    d.doc.first("summary").or_else(|| d.doc.first("content"))
}

rule! {
    pub DocPartialRequired {
        id: "doc-partial-required",
        category: Style,
        default: Off,
        description: "A partial type or method without a documentation comment (or with an empty one). \
                  Each part is checked on its own; StyleCop (SA1601) accepts one documented part, which \
                  needs the other parts of the project. Which elements need documentation follows \
                  StyleCop's defaults: everything but private members.",
        example: "public partial class A { }   // add a /// <summary> comment",
        kinds: &[
            kind::CLASS_DECLARATION,
            kind::STRUCT_DECLARATION,
            kind::INTERFACE_DECLARATION,
            kind::RECORD_DECLARATION,
            kind::RECORD_STRUCT_DECLARATION,
            kind::METHOD_DECLARATION,
        ],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(kind) = Decl::of(node.kind()) else {
            return;
        };
        let decl = Declaration {
            node: node.clone(),
            kind,
        };
        if !decl.is_partial_element() || !decl.needs_comment() {
            return;
        }
        if doc_of(node, ctx.file.grammar).is_some_and(|d| !d.is_blank()) {
            return;
        }
        for range in decl.names() {
            ctx.report(range, "partial elements should be documented");
        }
    }
}

rule! {
    pub DocSummaryRequired {
        id: "doc-summary-required",
        category: Style,
        default: Off,
        description: "A documentation comment without `<summary>` (and without `<inheritdoc />`) on a type, \
                  member or field that StyleCop's defaults ask to document: everything but private \
                  members. Partial types and methods are `doc-partial-summary-required`. No fix.",
        example: "/// <remarks>x</remarks>\npublic int Count;   // add a <summary>",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::takes_summary) else {
            return;
        };
        if d.is_partial_element() || !d.needs_comment() {
            return;
        }
        if d.doc.first("summary").is_some() || d.doc.has_inheritdoc() {
            return;
        }
        for range in d.names() {
            ctx.report(range, "the documentation should have a <summary>");
        }
    }
}

rule! {
    pub DocPartialSummaryRequired {
        id: "doc-partial-summary-required",
        category: Style,
        default: Off,
        description: "A documentation comment of a partial type or method without `<summary>` or `<content>` \
                  (and without `<inheritdoc />`). Each part is checked on its own. No fix.",
        example: "/// <remarks>x</remarks>\npublic partial class A { }   // add a <summary>",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::takes_summary) else {
            return;
        };
        if !d.is_partial_element() || !d.needs_comment() || d.doc.has_inheritdoc() {
            return;
        }
        if summary_or_content(&d).is_some() {
            return;
        }
        for range in d.names() {
            ctx.report(
                range,
                "the documentation of a partial element should have a <summary>",
            );
        }
    }
}

rule! {
    pub DocSummaryText {
        id: "doc-summary-text",
        category: Style,
        default: Off,
        description: "A `<summary>` without text (`<summary></summary>`; `<summary />` is left alone, as StyleCop \
                  does). Partial types and methods are `doc-partial-summary-text`. No fix.",
        example: "/// <summary></summary>\npublic int Count;   // describe it",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::takes_summary) else {
            return;
        };
        if d.is_partial_element() {
            return;
        }
        if d.doc.first("summary").is_some_and(|s| s.is_blank(false)) {
            for range in d.names() {
                ctx.report(range, "the <summary> should have text");
            }
        }
    }
}

rule! {
    pub DocPartialSummaryText {
        id: "doc-partial-summary-text",
        category: Style,
        default: Off,
        description: "The `<summary>` (or `<content>`) of a partial type or method has no text. Each part is \
                  checked on its own. No fix.",
        example: "/// <summary></summary>\npublic partial class A { }   // describe it",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::takes_summary) else {
            return;
        };
        if !d.is_partial_element() || d.doc.has_inheritdoc() {
            return;
        }
        if summary_or_content(&d).is_some_and(|s| s.is_blank(false)) {
            for range in d.names() {
                ctx.report(range, "the <summary> of a partial element should have text");
            }
        }
    }
}

rule! {
    pub NoDefaultSummary {
        id: "no-default-summary",
        category: Style,
        default: Off,
        description: "A `<summary>` that starts with the text Visual Studio generates, \"Summary description \
                  for ...\". Skipped when the comment has `<inheritdoc />`. No fix.",
        example: "/// <summary>Summary description for Foo</summary>   // say what Foo is\nclass Foo { }",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::is_element) else {
            return;
        };
        if d.doc.has_inheritdoc() {
            return;
        }
        for summary in d.doc.all("summary") {
            let first = summary.content().into_iter().next();
            if let Some(text) = first.filter(|c| c.kind() == kind::XML_TEXT)
                && normalized_text(&text)
                    .trim_start()
                    .starts_with(DEFAULT_SUMMARY)
                {
                    ctx.report(summary.range(), "the <summary> is the default text");
                    return;
                }
        }
    }
}
