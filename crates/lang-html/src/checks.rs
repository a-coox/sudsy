//! The `meta` family: checks of an element's name and attributes against the element metadata
//! (`meta.rs`). Param `check`:
//! - `unknown-element`: a name that is no HTML element (what is inside `<svg>` and `<math>` is left out);
//! - `element-name`: a name that is neither an HTML element nor a valid custom element name
//!   (`a-b`: lowercase letters, digits, `-`, `.` and `_`, with a `-`);
//! - `deprecated-element`; `deprecated-attr`;
//! - `required-attr`: an attribute the element must have;
//! - `boolean-style`: a boolean attribute written with a value (`checked="checked"`);
//! - `empty-style`: an attribute that may be empty (`hidden`, `crossorigin`) written as `=""`;
//! - `allowed-values`: a value that is not one of those the attribute is limited to.
//!
//! The message has `{name}` (the element), `{attr}` and `{value}`.

use crate::dom;
use crate::generated::kind as k;
use crate::meta;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxNodeExt};

pub const FAMILY: Family = Family {
    name: "meta",
    build,
};

#[derive(Clone, Copy)]
enum Check {
    UnknownElement,
    ElementName,
    DeprecatedElement,
    DeprecatedAttr,
    RequiredAttr,
    BooleanStyle,
    EmptyStyle,
    AllowedValues,
}

struct Meta {
    meta: &'static RuleMeta,
    check: Check,
    message: String,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    use Check::*;
    let check = p.choice(
        "check",
        true,
        &[
            ("unknown-element", UnknownElement),
            ("element-name", ElementName),
            ("deprecated-element", DeprecatedElement),
            ("deprecated-attr", DeprecatedAttr),
            ("required-attr", RequiredAttr),
            ("boolean-style", BooleanStyle),
            ("empty-style", EmptyStyle),
            ("allowed-values", AllowedValues),
        ],
    )?;
    let rule = Meta {
        meta,
        check,
        message: p.text("message")?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Meta {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[k::START_TAG]
    }

    fn check(&self, tag: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let text = ctx.file.text;
        let Some((name, name_range)) = dom::tag_name(tag, text) else {
            return;
        };
        let say = |attr: &str, value: &str| {
            self.message
                .replace("{name}", &name)
                .replace("{attr}", attr)
                .replace("{value}", value)
        };
        match self.check {
            Check::UnknownElement => {
                if !name.contains(':') && !meta::has("elements", &name) && !foreign(tag, text) {
                    ctx.report(name_range, say("", ""));
                }
            }
            Check::ElementName => {
                if !meta::has("elements", &name) && !custom_name(&name) && !foreign(tag, text) {
                    ctx.report(name_range, say("", ""));
                }
            }
            Check::DeprecatedElement => {
                if meta::has("deprecated", &name) {
                    ctx.report(name_range, say("", ""));
                }
            }
            Check::RequiredAttr => {
                let attrs = dom::attrs(tag, text);
                for required in meta::required(&name) {
                    if !attrs.iter().any(|a| a.name == required) {
                        ctx.report(name_range, say(required, ""));
                    }
                }
            }
            _ => {
                for a in dom::attrs(tag, text) {
                    let value = a.value.unwrap_or("");
                    let wrong = match self.check {
                        Check::DeprecatedAttr => meta::flagged("deprecated-attr", &name, &a.name),
                        Check::BooleanStyle => {
                            a.value.is_some() && meta::flagged("boolean", &name, &a.name)
                        }
                        Check::EmptyStyle => {
                            value.is_empty()
                                && a.value.is_some()
                                && meta::flagged("omit", &name, &a.name)
                        }
                        _ => meta::allowed(&name, &a.name).is_some_and(|allowed| {
                            a.value.is_some()
                                && !value.is_empty()
                                && !allowed.iter().any(|v| v.eq_ignore_ascii_case(value))
                        }),
                    };
                    if wrong {
                        ctx.report(a.key, say(&a.name, value));
                    }
                }
            }
        }
    }
}

/// Is the element inside `<svg>` or `<math>`, whose elements the metadata does not list?
fn foreign(tag: &SyntaxNode, text: &str) -> bool {
    tag.ancestors()
        .skip(2)
        .filter(|a| a.kind() == k::ELEMENT)
        .any(|e| {
            e.child(k::START_TAG)
                .and_then(|t| dom::tag_name(&t, text))
                .is_some_and(|(n, _)| n == "svg" || n == "math")
        })
}

fn custom_name(name: &str) -> bool {
    name.contains('-')
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-._".contains(c))
}
