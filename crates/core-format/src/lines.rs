//! Where the line breaks of a tree are, to answer "does this part of the source span lines"
//! without scanning it: a scan of every list's text makes the cost of a file with nested
//! lists on one line proportional to its size times its nesting depth.

use core_syntax::{SyntaxNode, TextRange};

/// The offsets of every `\n` and `\r` of a tree, in order.
pub(crate) struct LineBreaks {
    /// Identity of the tree the offsets belong to.
    tree: (usize, u32),
    offsets: Vec<u32>,
}

/// What a tree is, for noticing that a [`LineBreaks`] belongs to another one.
fn identity(root: &SyntaxNode) -> (usize, u32) {
    let green = root.green();
    (
        std::ptr::from_ref::<rowan::GreenNodeData>(&green) as usize,
        green.text_len().into(),
    )
}

fn collect(node: &rowan::GreenNodeData, at: &mut u32, out: &mut Vec<u32>) {
    for child in node.children() {
        match child {
            rowan::NodeOrToken::Node(n) => collect(n, at, out),
            rowan::NodeOrToken::Token(t) => {
                let text = t.text();
                out.extend(
                    text.bytes()
                        .enumerate()
                        .filter(|(_, b)| matches!(b, b'\n' | b'\r'))
                        .map(|(i, _)| *at + i as u32),
                );
                *at += text.len() as u32;
            }
        }
    }
}

impl LineBreaks {
    pub(crate) fn of(root: &SyntaxNode) -> Self {
        let mut offsets = Vec::new();
        collect(&root.green(), &mut 0, &mut offsets);
        LineBreaks {
            tree: identity(root),
            offsets,
        }
    }

    pub(crate) fn is_of(&self, root: &SyntaxNode) -> bool {
        self.tree == identity(root)
    }

    /// Is there a line break in `range` (offsets from the start of the tree)?
    pub(crate) fn any_in(&self, range: TextRange) -> bool {
        let (start, end) = (u32::from(range.start()), u32::from(range.end()));
        let first = self.offsets.partition_point(|&p| p < start);
        self.offsets.get(first).is_some_and(|&p| p < end)
    }
}
