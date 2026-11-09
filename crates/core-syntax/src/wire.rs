//! Wire encoding of trees (version [`WIRE_VERSION`]).
//!
//! Token text is never sent: the receiver already has the source. A tree is
//! its elements in pre-order, each with a kind and one number:
//!
//! * token: its width in UTF-8 bytes;
//! * node: its number of children (tokens and nodes, trivia included).
//!
//! Whether an element is a token is part of the kind ([`SyntaxKind::is_token`]),
//! and a node's width is the sum of its children's, so nothing else is sent.
//! Embedded-language nodes need no special case: they are ordinary nodes whose
//! child is the root of a tree of another language.
//!
//! [`FlatTree`] is that pair of arrays; [`WireTree`] is its serialized form
//! (see docs/architecture/syntax-tree.md).

use crate::kind::SyntaxKind;
use crate::tree::{SyntaxNode, TreeBuilder};
use rowan::GreenNode;
use serde::{Deserialize, Serialize};

pub const WIRE_VERSION: u32 = 1;

/// Deepest nesting a decoded tree may have. Dropping a green tree recurses once per level inside
/// rowan, which a debug build on a 2 MB thread stack survives up to about 3000 levels; real code
/// nests a few hundred levels at most.
pub const MAX_DEPTH: usize = 2048;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WireError {
    #[error("unsupported wire version {0} (expected {WIRE_VERSION})")]
    Version(u32),
    #[error("kinds ({kinds}) and values ({values}) differ in length")]
    LengthMismatch { kinds: usize, values: usize },
    #[error("malformed arrays: {0}")]
    Malformed(String),
    #[error("tree is empty")]
    Empty,
    #[error("root must be a node")]
    RootIsToken,
    #[error("element {0} is outside the root node")]
    TrailingElements(usize),
    #[error("tree ends inside a node")]
    Truncated,
    #[error("tree nests deeper than {MAX_DEPTH} levels")]
    TooDeep,
    #[error("token width {width} at byte {offset} is not on a UTF-8 boundary or exceeds the text")]
    BadWidth { offset: usize, width: u32 },
    #[error("tree covers {covered} bytes but the text has {len}")]
    TextLength { covered: usize, len: usize },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlatTree {
    pub kinds: Vec<u16>,
    pub values: Vec<u32>,
}

impl FlatTree {
    pub fn from_tree(root: &SyntaxNode) -> FlatTree {
        let mut flat = FlatTree::default();
        for event in root.preorder_with_tokens() {
            if let rowan::WalkEvent::Enter(el) = event {
                flat.kinds.push(el.kind().0);
                flat.values.push(match &el {
                    rowan::NodeOrToken::Node(n) => n.children_with_tokens().count() as u32,
                    rowan::NodeOrToken::Token(t) => u32::from(t.text_range().len()),
                });
            }
        }
        flat
    }

    /// Rebuilds the green tree, slicing token text out of `text`.
    pub fn build(&self, text: &str) -> Result<GreenNode, WireError> {
        if self.kinds.len() != self.values.len() {
            return Err(WireError::LengthMismatch {
                kinds: self.kinds.len(),
                values: self.values.len(),
            });
        }
        if self.kinds.is_empty() {
            return Err(WireError::Empty);
        }
        let mut builder = TreeBuilder::new();
        let mut open: Vec<u32> = Vec::new(); // children still expected per open node
        let mut offset = 0usize;
        for (i, (&k, &v)) in self.kinds.iter().zip(&self.values).enumerate() {
            let kind = SyntaxKind(k);
            if i == 0 {
                if kind.is_token() {
                    return Err(WireError::RootIsToken);
                }
            } else {
                match open.last_mut() {
                    Some(left) => *left -= 1,
                    None => return Err(WireError::TrailingElements(i)),
                }
            }
            if kind.is_token() {
                let end = offset.saturating_add(v as usize);
                let slice = text
                    .get(offset..end)
                    .ok_or(WireError::BadWidth { offset, width: v })?;
                builder.token(kind, slice);
                offset = end;
            } else {
                if open.len() >= MAX_DEPTH {
                    return Err(WireError::TooDeep);
                }
                builder.start_node(kind);
                open.push(v);
            }
            while open.last() == Some(&0) {
                open.pop();
                builder.finish_node();
            }
        }
        if !open.is_empty() {
            return Err(WireError::Truncated);
        }
        if offset != text.len() {
            return Err(WireError::TextLength {
                covered: offset,
                len: text.len(),
            });
        }
        Ok(builder.finish())
    }

    pub fn to_wire(&self) -> WireTree {
        let kinds = self.kinds.iter().flat_map(|k| k.to_le_bytes()).collect();
        let mut values = Vec::with_capacity(self.values.len());
        for &v in &self.values {
            write_varint(&mut values, v);
        }
        WireTree {
            version: WIRE_VERSION,
            kinds,
            values,
        }
    }
}

/// The serialized form: two byte arrays, embedded as `bin` fields in the
/// protocol's MessagePack message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTree {
    pub version: u32,
    /// Kinds as little-endian `u16`.
    #[serde(with = "serde_bytes")]
    pub kinds: Vec<u8>,
    /// Values as LEB128 varints.
    #[serde(with = "serde_bytes")]
    pub values: Vec<u8>,
}

impl WireTree {
    pub fn unpack(&self) -> Result<FlatTree, WireError> {
        if self.version != WIRE_VERSION {
            return Err(WireError::Version(self.version));
        }
        if !self.kinds.len().is_multiple_of(2) {
            return Err(WireError::Malformed("odd kinds length".into()));
        }
        let kinds: Vec<u16> = self
            .kinds
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        let mut values = Vec::with_capacity(kinds.len());
        let mut rest = &self.values[..];
        while !rest.is_empty() {
            let (v, n) =
                read_varint(rest).ok_or_else(|| WireError::Malformed("bad varint".into()))?;
            values.push(v);
            rest = &rest[n..];
        }
        Ok(FlatTree { kinds, values })
    }

    /// Unpacks and builds the tree in one step.
    pub fn decode(&self, text: &str) -> Result<GreenNode, WireError> {
        self.unpack()?.build(text)
    }
}

fn write_varint(out: &mut Vec<u8>, mut v: u32) {
    while v >= 0x80 {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn read_varint(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut v = 0u32;
    for (i, &b) in bytes.iter().enumerate().take(5) {
        v |= ((b & 0x7f) as u32).checked_shl(7 * i as u32)?;
        if b & 0x80 == 0 {
            return Some((v, i + 1));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::LanguageId;
    use proptest::prelude::*;

    #[derive(Clone, Debug)]
    enum T {
        Node(u16, Vec<T>),
        Token(u16, String),
    }

    fn node_kind(i: u16) -> SyntaxKind {
        let lang = [LanguageId::CORE, LanguageId::JSON, LanguageId::CSHARP][i as usize % 3];
        SyntaxKind::new(lang, false, i % 40)
    }
    fn token_kind(i: u16) -> SyntaxKind {
        SyntaxKind::new(LanguageId::new(1 + (i % 3) as u8), true, i % 300)
    }

    fn arb_tree() -> impl Strategy<Value = T> {
        let leaf = (any::<u16>(), "\\PC{0,6}").prop_map(|(k, s)| T::Token(k, s));
        let tree = leaf.prop_recursive(5, 80, 6, |inner| {
            (any::<u16>(), prop::collection::vec(inner, 0..6)).prop_map(|(k, c)| T::Node(k, c))
        });
        (any::<u16>(), prop::collection::vec(tree, 0..6)).prop_map(|(k, c)| T::Node(k, c))
    }

    fn emit(t: &T, b: &mut TreeBuilder) {
        match t {
            T::Node(k, c) => {
                b.start_node(node_kind(*k));
                c.iter().for_each(|x| emit(x, b));
                b.finish_node();
            }
            T::Token(k, s) => b.token(token_kind(*k), s),
        }
    }

    proptest! {
        #[test]
        fn encode_decode_round_trips(t in arb_tree()) {
            let mut b = TreeBuilder::new();
            emit(&t, &mut b);
            let green = b.finish();
            let root = SyntaxNode::new_root(green.clone());
            let text = crate::print(&root);

            let wire = FlatTree::from_tree(&root).to_wire();
            let bytes = rmp_serde::to_vec(&wire).unwrap();
            let wire: WireTree = rmp_serde::from_slice(&bytes).unwrap();
            let decoded = wire.decode(&text).unwrap();
            prop_assert_eq!(&decoded, &green);
            prop_assert_eq!(crate::print(&SyntaxNode::new_root(decoded)), text);
        }
    }

    #[test]
    fn rejects_corrupt_input() {
        let mut b = TreeBuilder::new();
        b.start_node(SyntaxKind::ERROR);
        b.token(SyntaxKind::new(LanguageId::JSON, true, 0), "héllo");
        b.finish_node();
        let root = SyntaxNode::new_root(b.finish());
        let flat = FlatTree::from_tree(&root);
        let text = crate::print(&root);
        assert!(flat.build(&text).is_ok());
        assert_eq!(
            flat.build("héllo!").unwrap_err(),
            WireError::TextLength { covered: 6, len: 7 }
        );
        let mut cut = flat.clone();
        cut.values[1] = 2; // splits the two-byte 'é'
        assert!(matches!(cut.build(&text), Err(WireError::BadWidth { .. })));
        let mut extra = flat.clone();
        extra.kinds.push(flat.kinds[1]);
        extra.values.push(0);
        assert!(matches!(
            extra.build(&text),
            Err(WireError::TrailingElements(2))
        ));
        let mut open = flat.clone();
        open.values[0] = 2;
        assert_eq!(open.build(&text).unwrap_err(), WireError::Truncated);
        let mut wire = flat.to_wire();
        wire.version = 99;
        assert_eq!(wire.unpack().unwrap_err(), WireError::Version(99));
    }
}
