//! The `doc-element` rule family: a documentation element that must exist (`check: required`),
//! have text (`text`) or declare a `name` (`name`). Params:
//! - `check`, `element`: what is checked and the element's tag.
//! - `scope`: the declarations checked: `element` (StyleCop's element declarations), `summary`
//!   (those that take a summary), `property`, `returning` (methods and delegates) or `generic`
//!   (types, methods and delegates that have type parameters).
//! - `partial`: `any` (default), `no` or `only`: whether partial types and methods are checked.
//! - `skip-inheritdoc`: `true` leaves comments with `<inheritdoc />` alone.
//! - `fallback`: an element read when `element` is missing (partial elements' `<content>`).
//! - `report`: where the diagnostic points: `names` (the declaration's names, for the first
//!   element; `<x />` has text, as in StyleCop), `return-type`, `element` (every element; `<x />`
//!   has no text), `attribute` (its `name` attribute) or `identifier` (inside an empty `name=""`).
//!
//! `required` skips members that need no comment, and `returning` ones that return void.

use super::doc_comments::{DOC_KINDS, Decl, Documented, Element, documented};
use super::kind;
use core_rules::tree::significant_range;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};

pub const FAMILY: Family = Family {
    name: "doc-element",
    build,
};

#[derive(Clone, Copy, PartialEq)]
enum Check {
    Text,
    Required,
    Name,
}

#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Element,
    Summary,
    Property,
    Returning,
    Generic,
}

#[derive(Clone, Copy, PartialEq)]
enum Partial {
    Any,
    No,
    Only,
}

#[derive(Clone, Copy, PartialEq)]
enum Report {
    Names,
    ReturnType,
    Element,
    Attribute,
    Identifier,
}

struct DocElement {
    meta: &'static RuleMeta,
    message: String,
    check: Check,
    element: String,
    scope: Scope,
    partial: Partial,
    skip_inheritdoc: bool,
    fallback: Option<String>,
    report: Report,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let rule = DocElement {
        meta,
        message: p.text("message")?,
        check: p.choice(
            "check",
            true,
            &[
                ("text", Check::Text),
                ("required", Check::Required),
                ("name", Check::Name),
            ],
        )?,
        element: p.text("element")?,
        scope: p.choice(
            "scope",
            true,
            &[
                ("element", Scope::Element),
                ("summary", Scope::Summary),
                ("property", Scope::Property),
                ("returning", Scope::Returning),
                ("generic", Scope::Generic),
            ],
        )?,
        partial: p.choice(
            "partial",
            false,
            &[
                ("any", Partial::Any),
                ("no", Partial::No),
                ("only", Partial::Only),
            ],
        )?,
        skip_inheritdoc: p.flag("skip-inheritdoc")?,
        fallback: p.optional("fallback"),
        report: p.choice(
            "report",
            true,
            &[
                ("names", Report::Names),
                ("return-type", Report::ReturnType),
                ("element", Report::Element),
                ("attribute", Report::Attribute),
                ("identifier", Report::Identifier),
            ],
        )?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

/// Types, methods and delegates: where `<typeparam>` is read (StyleCop's SA1621/SA1622 do not
/// look at records).
fn generic(k: Decl) -> bool {
    matches!(
        k,
        Decl::Class | Decl::Struct | Decl::Interface | Decl::Delegate | Decl::Method
    )
}

/// Where a diagnostic about a `name` attribute points: the attribute, else the element.
fn attribute_position(element: &Element) -> TextRange {
    match element.attr("name") {
        Some(attr) if attr.kind == kind::XML_NAME_ATTRIBUTE => attr.range,
        _ => element.range(),
    }
}

/// Where a diagnostic about a missing type parameter name points: inside the quotes of an
/// empty `name=""`, else the element.
fn identifier_position(element: &Element) -> TextRange {
    match element.attr("name") {
        Some(attr) if attr.kind == kind::XML_NAME_ATTRIBUTE => {
            TextRange::at(attr.range.end() - TextSize::from(1), TextSize::from(1))
        }
        _ => element.range(),
    }
}

impl DocElement {
    fn report_names(&self, d: &Documented, ctx: &mut RuleContext<'_>) {
        for range in d.names() {
            ctx.report(range, self.message.clone());
        }
    }

    fn first<'a>(&self, d: &'a Documented) -> Option<&'a Element> {
        let fallback = self.fallback.as_deref();
        d.doc
            .first(&self.element)
            .or_else(|| fallback.and_then(|f| d.doc.first(f)))
    }
}

impl Rule for DocElement {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        DOC_KINDS
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let accepts = |k: Decl| match self.scope {
            Scope::Element => k.is_element(),
            Scope::Summary => k.takes_summary(),
            Scope::Property => k == Decl::Property,
            Scope::Returning => k.has_return_type(),
            Scope::Generic => generic(k),
        };
        let Some(d) = documented(node, ctx, accepts) else {
            return;
        };
        let skip = match self.partial {
            Partial::Any => false,
            Partial::No => d.is_partial_element(),
            Partial::Only => !d.is_partial_element(),
        };
        if skip
            || (self.skip_inheritdoc && d.doc.has_inheritdoc())
            || (self.scope == Scope::Generic && d.type_parameters().is_empty())
        {
            return;
        }
        let grammar = ctx.file.grammar;
        match self.check {
            Check::Required => {
                let void = self.scope == Scope::Returning && d.returns_void(grammar);
                if !d.needs_comment() || void || self.first(&d).is_some() {
                    return;
                }
                match self.report {
                    Report::ReturnType => {
                        if let Some(ty) = d.return_type(grammar) {
                            ctx.report(significant_range(&ty, grammar), self.message.clone());
                        }
                    }
                    _ => self.report_names(&d, ctx),
                }
            }
            Check::Text if self.report == Report::Names => {
                if self.first(&d).is_some_and(|e| e.is_blank(false)) {
                    self.report_names(&d, ctx);
                }
            }
            Check::Text => {
                for e in d.doc.all(&self.element).filter(|e| e.is_blank(true)) {
                    ctx.report(e.range(), self.message.clone());
                }
            }
            Check::Name => {
                for e in d
                    .doc
                    .all(&self.element)
                    .filter(|e| e.declared_name().is_none())
                {
                    let range = match self.report {
                        Report::Identifier => identifier_position(e),
                        _ => attribute_position(e),
                    };
                    ctx.report(range, self.message.clone());
                }
            }
        }
    }
}
