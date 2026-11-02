//! The grammar tags the formatter asks about for every node, answered from a table built once
//! instead of scanning the tag strings of the kind.

use core_syntax::{Grammar, SyntaxKind};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub enum Tag {
    BinaryChain = 0,
    MemberChain,
    Assignment,
    InitializerExpression,
    Statement,
    TypeDeclaration,
    Namespace,
    DelimitedList,
    SwitchLabel,
}

const NAMES: [&str; 9] = [
    "binary-chain",
    "member-chain",
    "assignment",
    "initializer-expression",
    "statement",
    "type-declaration",
    "namespace",
    "delimited-list",
    "switch-label",
];

static TABLE: OnceLock<Vec<u16>> = OnceLock::new();

fn table(g: &Grammar) -> &'static [u16] {
    TABLE.get_or_init(|| {
        g.nodes()
            .iter()
            .map(|d| {
                NAMES
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| d.has_tag(n))
                    .fold(0u16, |acc, (i, _)| acc | 1 << i)
            })
            .collect()
    })
}

/// Does the node kind carry the tag?
pub fn has(g: &Grammar, kind: SyntaxKind, tag: Tag) -> bool {
    !kind.is_token()
        && kind.lang() == g.language_id
        && table(g)
            .get(kind.index() as usize)
            .is_some_and(|flags| flags & (1 << tag as u16) != 0)
}
