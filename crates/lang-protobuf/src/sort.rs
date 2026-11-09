//! The file header order of `buf format`: syntax or edition, package, imports (`public` first,
//! `option` last, by path), file options (built-in before custom, by name), then everything else
//! as written. Each statement moves with its comments (the trivia in front of it and a comment on
//! its last line); inside the import and option groups the blank lines of the source go.

use crate::generated::kind as k;
use core_syntax::SyntaxNode;

/// The group of a top-level statement: syntax or edition 0, package 1, import 2, option 3, rest 4.
pub fn group(item: &SyntaxNode) -> u8 {
    match item.kind() {
        k::SYNTAX | k::EDITION => 0,
        k::PACKAGE => 1,
        k::IMPORT => 2,
        k::OPTION_DECL => 3,
        _ => 4,
    }
}

/// The significant tokens of a statement (not its comments), as text.
fn words(item: &SyntaxNode) -> Vec<String> {
    item.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !matches!(t.kind(), k::WHITESPACE | k::NEWLINE | k::COMMENT))
        .map(|t| t.text().to_string())
        .collect()
}

fn key(item: &SyntaxNode) -> (u8, u8, String) {
    let words = words(item);
    match group(item) {
        2 => {
            let rank = match words.get(1).map(String::as_str) {
                Some("public") => 0,
                Some("weak") => 2,
                Some("option") => 3,
                _ => 1,
            };
            let path = words.iter().find(|w| w.starts_with(['"', '\'']));
            (2, rank, path.cloned().unwrap_or_default())
        }
        3 => {
            let name: String = words
                .iter()
                .skip(1)
                .take_while(|w| *w != "=")
                .cloned()
                .collect();
            (3, u8::from(name.starts_with('(')), name)
        }
        g => (g, 0, String::new()),
    }
}

/// The source with the statements of the file header in buf's order and the blank lines inside
/// the import and option groups gone, or `None` when it is already so.
pub fn sort_header(tree: &SyntaxNode, text: &str) -> Option<String> {
    let items: Vec<SyntaxNode> = tree.children().collect();
    let keys: Vec<_> = items.iter().map(key).collect();
    let mut order: Vec<usize> = (0..items.len()).collect();
    // Stable: statements with the same key (all the rest) keep their order.
    order.sort_by(|&a, &b| keys[a].cmp(&keys[b]));
    let piece = |p: usize| &text[items[order[p]].text_range()];
    // Two neighbours of the import or option group: the blanks between them go.
    let joined = |p: usize| {
        p > 0 && (2..=3).contains(&keys[order[p]].0) && keys[order[p - 1]].0 == keys[order[p]].0
    };
    let blank_lines = |p: usize| {
        let (a, b) = (piece(p - 1), piece(p));
        let tail = &a[a.trim_end().len()..];
        let head = &b[..b.len() - b.trim_start().len()];
        tail.matches('\n').count() + head.matches('\n').count() > 1
    };
    let moved = order.iter().enumerate().any(|(p, &o)| p != o);
    if !moved && !(1..order.len()).any(|p| joined(p) && blank_lines(p)) {
        return None;
    }
    let mut out = String::with_capacity(text.len() + 8);
    for p in 0..order.len() {
        let mut s = piece(p);
        if joined(p) {
            out.truncate(out.trim_end().len());
            s = s.trim_start();
        }
        if !out.is_empty() && !out.ends_with('\n') && !s.starts_with(['\n', '\r']) {
            out.push('\n');
        }
        out.push_str(s);
    }
    let end = items
        .last()
        .map_or(0, |n| usize::from(n.text_range().end()));
    out.push_str(&text[end..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use provider_api::{LanguageProvider, SourceFile};

    #[test]
    fn orders_the_header_and_drops_blank_lines_inside_groups() {
        let src = "syntax = \"proto3\";\noption b = 1;\nimport \"z\";\n\nimport option \"o\";\nimport \"a\";\n// p\npackage p;\noption a = 1; // t\noption (c) = 1;\n\nmessage M {}\n";
        let tree = crate::provider()
            .parse(&SourceFile::new("a.proto", 1, src))
            .unwrap()
            .tree;
        assert_eq!(
            super::sort_header(&tree, src).unwrap(),
            "syntax = \"proto3\";\n// p\npackage p;\nimport \"a\";\nimport \"z\";\nimport option \"o\";\noption a = 1; // t\noption b = 1;\noption (c) = 1;\n\nmessage M {}\n"
        );
    }
}
