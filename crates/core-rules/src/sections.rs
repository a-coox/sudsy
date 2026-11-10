//! `no-duplicate-sections`, written against the `#sections` (a file's container of sections) and
//! `#section` (a section's header node, whose `name` slot holds the name token) grammar tags.

use crate::rule;
use crate::rule::RuleContext;
use core_syntax::{SyntaxNode, TextRange};
use std::collections::HashMap;

rule! {
    pub NoDuplicateSections {
        id: "no-duplicate-sections",
        category: Correctness,
        default: Warn,
        description: "Two sections of one file have the same name (EditorConfig: glob). Readers \
                  merge them or keep only one, so the settings of the other may be lost; merge them.",
        example: "[core]\nbare = false\n[core]   # reported: the same section again",
        tags: &["sections"],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let grammar = ctx.file.grammar;
        let mut seen: HashMap<String, TextRange> = HashMap::new();
        let headers = node
            .children()
            .filter_map(|section| section.children().next())
            .filter(|h| grammar.has_tag(h.kind(), "section"));
        for header in headers {
            let Some(name) = grammar.slot(&header, "name").and_then(|n| n.into_token()) else {
                continue;
            };
            let (text, range) = (name.text().to_string(), name.text_range());
            if let Some(first) = seen.get(&text) {
                ctx.report(range, format!("section `[{text}]` is already defined"))
                    .related(*first, "first defined here");
            } else {
                seen.insert(text, range);
            }
        }
    }
}
