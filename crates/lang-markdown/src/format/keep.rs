//! Blocks and files kept as written.

use super::*;

impl Render<'_> {
    /// Blocks kept as written: a tab's width depends on its column, and a fence with no closing
    /// fence would gain one.
    pub(super) fn keep(&self, e: &SyntaxElement, top: bool) -> bool {
        let range = e.text_range();
        let from = self.line_start(usize::from(range.start()));
        let open_fence = matches!(e, SyntaxElement::Node(n) if n.kind() == k::FENCED_CODE
            && n.children_with_tokens().filter(|c| c.kind() == k::FENCE).count() < 2);
        // Prettier keeps setext headings as written.
        let setext = matches!(e, SyntaxElement::Node(n) if n.kind() == k::HEADING
            && atx_level(self.src, n).is_none());
        open_fence
            || setext
            || top && self.src[from..usize::from(range.end())].contains('\t')
            || top
                && matches!(e, SyntaxElement::Node(n)
                if (n.kind() == k::BLOCK_QUOTE || n.kind() == k::LIST)
                    && significant(n).any(|t| t.kind() == k::INDENTED_CODE))
            || top && matches!(e, SyntaxElement::Node(n) if self.irregular(n))
            || matches!(e, SyntaxElement::Node(n) if n.kind() == k::TABLE && self.ragged(n))
    }

    /// Indentation the renderer would remove or add although it carries meaning: an indented
    /// or lazy paragraph line that would start a block at its container's indentation
    /// (`Foo\n    ---` is not a heading, a lazy `===` is one once it gets the quote's `>`), an indented
    /// opening fence (its content loses as much), a list followed by an indented line (new
    /// marker widths could pull it into the last item).
    pub(super) fn irregular(&self, n: &SyntaxNode) -> bool {
        let indented = |s: &str| s.starts_with([' ', '\t']);
        if n.kind() == k::LIST
            && self.src[usize::from(n.text_range().end())..]
                .lines()
                .find(|l| !l.trim().is_empty())
                .is_some_and(indented)
        {
            return true;
        }
        n.descendants().any(|d| match d.kind() {
            k::INLINE => {
                let chain = self.chain(&d);
                let text = &self.src[d.text_range()];
                text.split('\n').skip(1).any(|l| {
                    let lazy = !in_container(l, &chain);
                    (lazy || indented(strip(l, &chain))) && starts_block(l.trim())
                })
            }
            k::FENCED_CODE => significant(&d)
                .find(|t| t.kind() == k::FENCE)
                .is_some_and(|t| indented(t.text())),
            _ => false,
        })
    }

    /// Files whose blocks cannot all be told apart or rendered again: an empty heading has no
    /// token to find its level in, and an HTML block of the kinds that run on past blank lines
    /// but was cut short by the parser (what follows is its text, not Markdown).
    pub(super) fn unformattable(&self, doc: &SyntaxNode) -> bool {
        doc.descendants()
            .any(|n| n.kind() == k::HEADING && significant(&n).next().is_none())
            || doc
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .any(|t| {
                    t.kind() == k::HTML_BLOCK
                        && t.parent().is_some_and(|p| p.kind() == k::HTML)
                        && {
                            let low = t.text().to_ascii_lowercase();
                            ["<script", "<pre", "<style", "<textarea"].iter().any(|o| {
                                low.starts_with(o) && !low.contains(&format!("</{}", &o[1..]))
                            })
                        }
                })
    }
}
