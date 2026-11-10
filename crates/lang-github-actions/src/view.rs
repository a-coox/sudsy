//! A small reading API over the YAML tree: mappings, sequences and scalars of a workflow.

use core_syntax::{SyntaxNode, TextRange};
use lang_yaml::generated::kind;
use lang_yaml::scalar::{double_quoted_value, plain_value, single_quoted_value};

pub struct Entry {
    pub key: String,
    pub key_range: TextRange,
    pub value: Option<SyntaxNode>,
}

/// The node under a property node (anchor, tag), else the node itself.
pub fn inner(node: &SyntaxNode) -> Option<SyntaxNode> {
    match node.kind() {
        kind::PROPERTY_NODE => node.children().next(),
        _ => Some(node.clone()),
    }
}

/// The text and range of a scalar node (`None` for anything else).
pub fn scalar(node: &SyntaxNode, text: &str) -> Option<(String, TextRange)> {
    let node = inner(node)?;
    if node.kind() != kind::SCALAR {
        return None;
    }
    let token = node
        .children_with_tokens()
        .filter_map(|c| c.into_token())
        .find(|t| {
            matches!(
                t.kind(),
                kind::PLAIN | kind::SINGLE_QUOTED | kind::DOUBLE_QUOTED
            )
        })?;
    let raw = &text[token.text_range()];
    let value = match token.kind() {
        kind::PLAIN => plain_value(raw),
        kind::SINGLE_QUOTED => single_quoted_value(raw),
        kind::DOUBLE_QUOTED => double_quoted_value(raw)?,
        _ => return None,
    };
    Some((value, token.text_range()))
}

/// The `key: value` pairs of a block or flow mapping.
pub fn entries(node: &SyntaxNode, text: &str) -> Vec<Entry> {
    let Some(node) = inner(node) else {
        return Vec::new();
    };
    if !matches!(node.kind(), kind::BLOCK_MAPPING | kind::FLOW_MAPPING) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in node
        .children()
        .filter(|c| matches!(c.kind(), kind::BLOCK_MAPPING_ENTRY | kind::FLOW_PAIR))
    {
        let mut after_colon = false;
        let (mut key, mut value) = (None, None);
        for child in entry.children_with_tokens() {
            if child.kind() == kind::COLON {
                after_colon = true;
            } else if let Some(n) = child.into_node() {
                *(if after_colon { &mut value } else { &mut key }) = Some(n);
            }
        }
        if let Some((name, key_range)) = key.and_then(|k| scalar(&k, text)) {
            out.push(Entry {
                key: name,
                key_range,
                value,
            });
        }
    }
    out
}

pub fn get(node: &SyntaxNode, text: &str, key: &str) -> Option<Entry> {
    entries(node, text).into_iter().find(|e| e.key == key)
}

/// The values of a block or flow sequence.
pub fn items(node: &SyntaxNode) -> Vec<SyntaxNode> {
    let Some(node) = inner(node) else {
        return Vec::new();
    };
    match node.kind() {
        kind::BLOCK_SEQUENCE => node
            .children()
            .filter_map(|e| e.children().next())
            .collect(),
        kind::FLOW_SEQUENCE => node.children().collect(),
        _ => Vec::new(),
    }
}

/// The mapping a file is made of (its first document's body).
pub fn root_mapping(root: &SyntaxNode) -> Option<SyntaxNode> {
    root.children()
        .find(|c| c.kind() == kind::DOCUMENT)?
        .children()
        .next()
}

/// A scalar or a sequence of scalars (`needs: a`, `needs: [a, b]`), with ranges.
pub fn scalar_list(node: &SyntaxNode, text: &str) -> Vec<(String, TextRange)> {
    match scalar(node, text) {
        Some(one) => vec![one],
        None => items(node).iter().filter_map(|n| scalar(n, text)).collect(),
    }
}
