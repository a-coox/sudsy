//! Rules about the `<value>`, `<param>`, `<returns>` and `<typeparam>` elements (SA1610,
//! SA1613-SA1617, SA1619, SA1621, SA1622).

use super::doc_comments::{DOC_KINDS, Decl, Documented, documented};
use core_rules::rule;
use core_rules::tree::significant_range;
use core_rules::{Edit, Fix, FixKind, RuleContext};
use core_syntax::{SyntaxNode, TextRange};

/// Types, methods and delegates with type parameters: where `<typeparam>` is read (StyleCop's
/// SA1621/SA1622 do not look at records).
fn generic(k: Decl) -> bool {
    matches!(
        k,
        Decl::Class | Decl::Struct | Decl::Interface | Decl::Delegate | Decl::Method
    )
}

fn has_type_parameters(d: &Documented) -> bool {
    !d.type_parameters().is_empty()
}

/// Where a diagnostic about a `name` attribute points: the attribute, else the element.
fn name_position(element: &super::doc_comments::Element) -> TextRange {
    match element.attr("name") {
        Some(attr) if attr.kind == super::kind::XML_NAME_ATTRIBUTE => attr.range,
        _ => element.range(),
    }
}

/// Where a diagnostic about a missing type parameter name points: inside the quotes of an
/// empty `name=""`, else the element.
fn identifier_position(element: &super::doc_comments::Element) -> TextRange {
    match element.attr("name") {
        Some(attr) if attr.kind == super::kind::XML_NAME_ATTRIBUTE => {
            let inside = attr.range.end() - core_syntax::TextSize::from(1);
            TextRange::at(inside, core_syntax::TextSize::from(1))
        }
        _ => element.range(),
    }
}

rule! {
    pub DocValueText {
        id: "doc-value-text",
        category: Style,
        default: Off,
        description: "A property's `<value>` without text (`<value></value>`; `<value />` is left alone, as \
                  StyleCop does). Skipped when the comment has `<inheritdoc />`. No fix.",
        example: "/// <value></value>\npublic int Count { get; }   // describe it",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, |k| k == Decl::Property) else {
            return;
        };
        if d.doc.has_inheritdoc() {
            return;
        }
        if d.doc.first("value").is_some_and(|v| v.is_blank(false)) {
            for range in d.names() {
                ctx.report(range, "the <value> should have text");
            }
        }
    }
}

rule! {
    pub DocParamHasName {
        id: "doc-param-has-name",
        category: Style,
        default: Off,
        description: "A `<param>` without a `name` attribute, or with an empty one. No fix.",
        example: "/// <param>The value.</param>   // write <param name=\"value\">\nvoid Set(int value) { }",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::is_element) else {
            return;
        };
        for param in d.doc.all("param") {
            if param.declared_name().is_none() {
                ctx.report(name_position(param), "the <param> should declare the parameter name");
            }
        }
    }
}

rule! {
    pub DocParamHasText {
        id: "doc-param-has-text",
        category: Style,
        default: Off,
        description: "A `<param>` without text. Skipped when the comment has `<inheritdoc />`. No fix.",
        example: "/// <param name=\"value\"></param>   // describe the parameter\nvoid Set(int value) { }",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::is_element) else {
            return;
        };
        if d.doc.has_inheritdoc() {
            return;
        }
        for param in d.doc.all("param") {
            if param.is_empty_element() || param.is_blank(false) {
                ctx.report(param.range(), "the <param> should have text");
            }
        }
    }
}

rule! {
    pub DocReturnsRequired {
        id: "doc-returns-required",
        category: Style,
        default: Off,
        description: "A documented method or delegate that does not return `void` and has no `<returns>` (and \
                  no `<inheritdoc />`). Private members are left alone, as in StyleCop's defaults. No fix.",
        example: "/// <summary>Counts.</summary>\npublic int Count() => 0;   // add <returns>",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::has_return_type) else {
            return;
        };
        let grammar = ctx.file.grammar;
        if !d.needs_comment() || d.returns_void(grammar) || d.doc.has_inheritdoc() {
            return;
        }
        if d.doc.first("returns").is_some() {
            return;
        }
        if let Some(ty) = d.return_type(grammar) {
            ctx.report(
                significant_range(&ty, grammar),
                "the return value should be documented with <returns>",
            );
        }
    }
}

rule! {
    pub DocReturnsHasText {
        id: "doc-returns-has-text",
        category: Style,
        default: Off,
        description: "A `<returns>` without text. Skipped when the comment has `<inheritdoc />`. No fix.",
        example: "/// <returns></returns>   // describe the result\npublic int Count() => 0;",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::is_element) else {
            return;
        };
        if d.doc.has_inheritdoc() {
            return;
        }
        for returns in d.doc.all("returns") {
            if returns.is_empty_element() || returns.is_blank(false) {
                ctx.report(returns.range(), "the <returns> should have text");
            }
        }
    }
}

rule! {
    pub NoReturnsOnVoid {
        id: "no-returns-on-void",
        category: Style,
        default: Off,
        fix: Safe,
        description: "A `<returns>` on a method or delegate that returns `void`. The fix removes the element \
                  with its line when it is alone on it.",
        example: "/// <returns>Nothing.</returns>   // remove it\npublic void Run() { }",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::has_return_type) else {
            return;
        };
        if !d.returns_void(ctx.file.grammar) {
            return;
        }
        if let Some(returns) = d.doc.first("returns") {
            let range = returns.range();
            let removal = line_range(ctx.file.text, range);
            ctx.report(range, "a void member should not document a return value")
                .fix(Fix::new(
                    "Remove the <returns> element",
                    FixKind::Safe,
                    vec![Edit::delete(removal)],
                ));
        }
    }
}

/// The element's range, widened to the whole line when only `///` and white space share it.
fn line_range(text: &str, element: TextRange) -> TextRange {
    let (start, end) = (usize::from(element.start()), usize::from(element.end()));
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i + 1);
    let before = text[line_start..start].trim();
    let after = text[end..line_end].trim();
    if before == "///" && after.is_empty() {
        TextRange::new((line_start as u32).into(), (line_end as u32).into())
    } else {
        element
    }
}

rule! {
    pub DocPartialTypeparams {
        id: "doc-partial-typeparams",
        category: Style,
        default: Off,
        description: "A documented partial generic type (with a `<summary>`) that has no `<typeparam>` for one \
                  of its type parameters. Each part is checked on its own. No fix.",
        example: "/// <summary>Box.</summary>\npublic partial class Box<T> { }   // add <typeparam name=\"T\">",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, Decl::is_type) else {
            return;
        };
        if !d.is_partial() || !d.needs_comment() || d.doc.has_inheritdoc() {
            return;
        }
        if d.doc.first("summary").is_none() {
            return;
        }
        let documented_names: Vec<String> = d
            .doc
            .all("typeparam")
            .filter_map(|e| e.declared_name())
            .map(|a| a.value)
            .collect();
        for parameter in d.type_parameters() {
            if !documented_names.iter().any(|n| n == parameter.text()) {
                ctx.report(
                    parameter.text_range(),
                    format!("the type parameter '{}' should be documented", parameter.text()),
                );
            }
        }
    }
}

rule! {
    pub DocTypeparamHasName {
        id: "doc-typeparam-has-name",
        category: Style,
        default: Off,
        description: "A `<typeparam>` without a `name` attribute, or with an empty one, in the documentation of a \
                  generic class, struct, interface, method or delegate. No fix.",
        example: "/// <typeparam>The item.</typeparam>   // write <typeparam name=\"T\">\nvoid Add<T>() { }",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, generic) else {
            return;
        };
        if !has_type_parameters(&d) || d.doc.has_inheritdoc() {
            return;
        }
        for element in d.doc.all("typeparam") {
            if element.declared_name().is_none() {
                ctx.report(
                    identifier_position(element),
                    "the <typeparam> should declare the type parameter name",
                );
            }
        }
    }
}

rule! {
    pub DocTypeparamHasText {
        id: "doc-typeparam-has-text",
        category: Style,
        default: Off,
        description: "A `<typeparam>` without text (`<typeparam name=\"T\" />` counts), in the documentation of a \
                  generic class, struct, interface, method or delegate. No fix.",
        example: "/// <typeparam name=\"T\"></typeparam>   // describe the type parameter\nvoid Add<T>() { }",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, generic) else {
            return;
        };
        if !has_type_parameters(&d) || d.doc.has_inheritdoc() {
            return;
        }
        for element in d.doc.all("typeparam") {
            if element.is_blank(true) {
                ctx.report(element.range(), "the <typeparam> should have text");
            }
        }
    }
}
