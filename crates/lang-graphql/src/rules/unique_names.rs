use crate::generated::kind;
use core_rules::{RuleContext, rule};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};
use std::collections::HashMap;

/// Reports the second and later definition of a kind that has the same name as an earlier one.
fn report_duplicates(
    document: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
    definition: SyntaxKind,
    what: &str,
) {
    let mut seen: HashMap<String, TextRange> = HashMap::new();
    for def in document.children().filter(|c| c.kind() == definition) {
        let Some(name) = super::name_of(&def) else {
            continue;
        };
        let (text, range) = (name.text().to_string(), name.text_range());
        match seen.get(&text) {
            Some(first) => {
                ctx.report(range, format!("{what} `{text}` is already defined"))
                    .related(*first, "first defined here");
            }
            None => {
                seen.insert(text, range);
            }
        }
    }
}

rule! {
    pub UniqueOperationName {
        id: "unique-operation-name",
        category: Correctness,
        default: Error,
        description: "Two operations of a document have the same name; a server cannot tell which \
                  to run and tools that key on the name (caches, persisted queries) collide. \
                  Checked within the file.",
        example: "query A { a }\nquery A { b }   # reported",
        kinds: &[kind::DOCUMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        report_duplicates(node, ctx, kind::OPERATION, "operation");
    }
}

rule! {
    pub UniqueFragmentName {
        id: "unique-fragment-name",
        category: Correctness,
        default: Error,
        description: "Two fragments of a document have the same name; a spread cannot tell which \
                  one it means. Checked within the file.",
        example: "fragment F on T { a }\nfragment F on T { b }   # reported",
        kinds: &[kind::DOCUMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        report_duplicates(node, ctx, kind::FRAGMENT, "fragment");
    }
}
