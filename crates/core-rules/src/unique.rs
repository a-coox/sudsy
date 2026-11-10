//! Names that must not repeat: [`Repeats`], the first-seen bookkeeping of the rules that report a
//! repeat against its first occurrence, and the `unique-names` family built on it.

use crate::rule::{Rule, RuleContext, RuleMeta};
use crate::{Family, Params};
use core_syntax::{SyntaxElement, SyntaxNode, TextRange};
use std::collections::HashMap;
use std::hash::Hash;

/// The first range of each key.
#[derive(Default)]
pub struct Repeats<K>(HashMap<K, TextRange>);

impl<K: Hash + Eq> Repeats<K> {
    /// Remembers `range` as the first of `key`; a repeat is reported at `range` with `message()`
    /// and the first one as `related`.
    pub fn check(
        &mut self,
        ctx: &mut RuleContext<'_>,
        key: K,
        range: TextRange,
        message: impl FnOnce() -> String,
        related: &str,
    ) {
        if let Some(first) = self.0.get(&key) {
            ctx.report(range, message()).related(*first, related);
        } else {
            self.0.insert(key, range);
        }
    }
}

pub const FAMILY: Family = Family {
    name: "unique-names",
    build,
};

/// `container`: the grammar tag of the node that holds the items; `item`: the tag of the items
/// whose `name` slot must not repeat among them. `message` has `{name}` for the name.
struct UniqueNames {
    meta: &'static RuleMeta,
    message: String,
    tags: [&'static str; 1],
    item: String,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let rule = UniqueNames {
        meta,
        message: p.text("message")?,
        tags: [Box::leak(p.text("container")?.into_boxed_str())],
        item: p.text("item")?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for UniqueNames {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn tags(&self) -> &[&'static str] {
        &self.tags
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let grammar = ctx.file.grammar;
        let mut seen = Repeats::default();
        for item in node
            .children()
            .filter(|c| grammar.has_tag(c.kind(), &self.item))
        {
            let name = match grammar.slot(&item, "name") {
                Some(SyntaxElement::Token(t)) => Some(t),
                Some(SyntaxElement::Node(n)) => n.first_token(),
                None => None,
            };
            if let Some(name) = name {
                let text = name.text().to_string();
                let message = || self.message.replace("{name}", &text);
                let range = name.text_range();
                seen.check(ctx, text.clone(), range, message, "first defined here");
            }
        }
    }
}
