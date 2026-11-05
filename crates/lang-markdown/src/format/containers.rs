//! Containers (quotes, list items) and the source lines of blocks.

use super::*;

impl Render<'_> {
    pub(super) fn line_start(&self, off: usize) -> usize {
        self.src[..off].rfind('\n').map_or(0, |i| i + 1)
    }

    /// Absolute column where the content of a quote or list item starts.
    pub(super) fn content_col(&self, n: &SyntaxNode) -> usize {
        let marker = n
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == k::QUOTE_MARKER || t.kind() == k::LIST_MARKER);
        marker.map_or(0, |t| end(&t) - self.line_start(start(&t)))
    }

    /// The containers around `node`, outermost first.
    pub(super) fn chain(&self, node: &SyntaxNode) -> Vec<Cont> {
        let mut out = Vec::new();
        let mut parent_col = None;
        let mut ancestors: Vec<SyntaxNode> = node
            .ancestors()
            .skip(1)
            .filter(|a| a.kind() == k::BLOCK_QUOTE || a.kind() == k::LIST_ITEM)
            .collect();
        ancestors.reverse();
        for a in ancestors {
            let col = self.content_col(&a);
            if a.kind() == k::BLOCK_QUOTE {
                out.push(Cont::Quote);
            } else {
                out.push(Cont::Item(col.saturating_sub(parent_col.unwrap_or(0))));
            }
            parent_col = Some(col);
        }
        out
    }

    /// Chain for the content of `node` itself when it is a container.
    pub(super) fn chain_in(&self, node: &SyntaxNode) -> Vec<Cont> {
        let mut c = self.chain(node);
        let parent_col = node
            .ancestors()
            .skip(1)
            .find(|a| a.kind() == k::BLOCK_QUOTE || a.kind() == k::LIST_ITEM)
            .map_or(0, |a| self.content_col(&a));
        c.push(if node.kind() == k::BLOCK_QUOTE {
            Cont::Quote
        } else {
            Cont::Item(self.content_col(node).saturating_sub(parent_col))
        });
        c
    }

    /// The blocks of a container, as elements.
    pub(super) fn block_elems(parent: &SyntaxNode) -> Vec<SyntaxElement> {
        parent
            .children_with_tokens()
            .filter(|e| match e {
                SyntaxElement::Node(_) => true,
                SyntaxElement::Token(t) => [
                    k::INDENTED_CODE,
                    k::THEMATIC_BREAK,
                    k::LINK_DEF,
                    k::FRONT_MATTER,
                ]
                .contains(&t.kind()),
            })
            .collect()
    }

    /// The element as written, minus the prefixes of the containers around it.
    pub(super) fn verbatim(&self, e: &SyntaxElement) -> Vec<String> {
        let (Some(f), Some(l)) = elem_ends(e) else {
            return Vec::new();
        };
        let chain = match e {
            SyntaxElement::Node(n) => self.chain(n),
            SyntaxElement::Token(t) => self.chain_of_token(t),
        };
        let (mut from, mut to) = (start(&f), end(&l));
        if let SyntaxElement::Node(n) = e
            && n.kind() == k::HEADING
        {
            // The markers are trivia outside the text: an ATX heading's `#`s, a setext underline.
            if atx_level(self.src, n).is_some() {
                from = self.src[..from].trim_end_matches([' ', '\t', '#']).len();
            } else {
                to += self.src[to..].find('\n').map_or(0, |i| i + 1);
                to += self.src[to..].find('\n').unwrap_or(self.src.len() - to);
            }
        }
        // A top-level block keeps the indentation of its first line.
        let line = self.line_start(from);
        if chain.is_empty() && self.src[line..from].trim().is_empty() {
            from = line;
        }
        raw_lines(&self.src[from..to], &chain)
    }

    pub(super) fn chain_of_token(&self, t: &SyntaxToken) -> Vec<Cont> {
        t.parent().map_or(Vec::new(), |p| {
            let mut c = self.chain(&p);
            if p.kind() == k::BLOCK_QUOTE || p.kind() == k::LIST_ITEM {
                c = self.chain_in(&p);
            }
            c
        })
    }
}
