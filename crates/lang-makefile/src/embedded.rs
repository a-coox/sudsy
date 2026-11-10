//! The commands of a recipe are Shell code, one shell per line of the recipe. Make's own syntax in
//! them (`$(CC)`, `$@`, `$$`, the tab that starts each continued line) is carved out: the shell
//! never sees it.

use crate::generated::kind as k;
use core_syntax::{Embedded, SyntaxNode, TextRange, TextSize};

fn size(n: usize) -> TextSize {
    TextSize::try_from(n).expect("a file is under 4 GiB")
}

pub fn find(root: &SyntaxNode, text: &str) -> Vec<Embedded> {
    let mut out = Vec::new();
    for line in root.descendants().filter(|n| n.kind() == k::RECIPE_LINE) {
        let Some((whole, _)) = crate::format::code(&line) else {
            continue;
        };
        let src = &text[whole];
        let skip = src.len() - src.trim_start_matches(['@', '-', '+', ' ', '\t']).len();
        let range = TextRange::new(
            whole.start() + size(skip),
            whole.start() + size(src.trim_end().len()),
        );
        if range.is_empty() {
            continue;
        }
        let mut carved = Vec::new();
        let from = usize::from(range.start());
        for (i, _) in text[range].match_indices("\n\t") {
            carved.push(TextRange::at(size(from + i + 1), size(1)));
        }
        for e in line.descendants_with_tokens() {
            let r = e.text_range();
            match e.kind() {
                k::CALL | k::MAKE_REF => carved.push(r),
                k::MAKE_ESCAPE => carved.push(TextRange::at(r.start(), size(1))),
                _ => {}
            }
        }
        carved.retain(|r| range.contains_range(*r));
        carved.sort_by_key(|r| r.start());
        carved.dedup_by(|b, a| b.start() < a.end());
        out.push(Embedded {
            language: "shell".into(),
            range,
            carved,
            format: false,
        });
    }
    out
}
