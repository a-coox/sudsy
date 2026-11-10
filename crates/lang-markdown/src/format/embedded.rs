//! The code of fenced blocks: the info string's first word names the language. The container
//! prefix of each line (quote markers, list indentation) is the host's part of the region.

use super::Render;
use crate::generated::kind as k;
use core_syntax::embedded::size;
use core_syntax::{Embedded, SyntaxNode, SyntaxNodeExt, TextRange};

pub fn find(root: &SyntaxNode, text: &str) -> Vec<Embedded> {
    // The tree walk below allocates a red node per node; a file with no fence marker skips it.
    if !text.contains("```") && !text.contains("~~~") {
        return Vec::new();
    }
    let render = Render { src: text, tab: 2 };
    let at = size;
    let mut out = Vec::new();
    for n in root.descendants().filter(|n| n.kind() == k::FENCED_CODE) {
        let info = n.token(k::INFO);
        let language = info
            .as_ref()
            .and_then(|t| t.text().split_whitespace().next())
            .map(|w| w.trim_matches(['{', '}', '.']).to_lowercase());
        let (lines, closed) = render.fence_lines(&n);
        let (Some(language), Some(first), Some(last), true) =
            (language, lines.first(), lines.last(), closed)
        else {
            continue;
        };
        out.push(Embedded {
            language,
            range: TextRange::new(at(first.0), at(last.2)),
            carved: (lines.iter().filter(|l| l.1 > 0))
                .map(|&(start, prefix, _)| TextRange::new(at(start), at(start + prefix)))
                .collect(),
            format: true,
        });
    }
    out
}
