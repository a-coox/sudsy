use crate::generated::kind;
use core_rules::{RuleContext, rule};
use core_syntax::SyntaxNode;
use std::collections::HashMap;

/// The name of a section header, as written.
fn name(section: &SyntaxNode) -> Option<(String, core_syntax::TextRange)> {
    let header = section
        .children()
        .find(|c| c.kind() == kind::SECTION_HEADER)?;
    let token = header
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind::NAME)?;
    Some((token.text().to_string(), token.text_range()))
}

rule! {
    pub NoDuplicateSections {
        id: "no-duplicate-sections",
        category: Correctness,
        default: Warn,
        description: "Two sections of one file have the same name. Readers merge them or keep \
                  only one, so the settings of the other may be lost; merge them.",
        example: "[core]\nbare = false\n[core]   # reported: the same section again",
        kinds: &[kind::DOCUMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut seen: HashMap<String, core_syntax::TextRange> = HashMap::new();
        for (name, range) in node.children().filter(|c| c.kind() == kind::SECTION).filter_map(|s| name(&s)) {
            if let Some(first) = seen.get(&name) {
                ctx.report(range, format!("section `[{name}]` is already defined"))
                    .related(*first, "first defined here");
            } else {
                seen.insert(name, range);
            }
        }
    }
}
