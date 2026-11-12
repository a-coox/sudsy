//! GFM tables, columns aligned.

use super::*;

impl Render<'_> {
    pub(super) fn table(&self, n: &SyntaxNode) -> Vec<String> {
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut align: Vec<char> = Vec::new();
        for e in n.children_with_tokens() {
            match e {
                SyntaxElement::Token(t) if t.kind() == k::TABLE_DELIMITER => {
                    align = t
                        .text()
                        .trim()
                        .trim_matches('|')
                        .split('|')
                        .map(|c| {
                            let c = c.trim();
                            match (c.starts_with(':'), c.ends_with(':')) {
                                (true, true) => 'c',
                                (true, false) => 'l',
                                (false, true) => 'r',
                                _ => 'n',
                            }
                        })
                        .collect();
                }
                SyntaxElement::Node(r) if r.kind() == k::TABLE_ROW => rows.push(self.row(&r)),
                _ => {}
            }
        }
        let cols = rows
            .iter()
            .map(Vec::len)
            .chain([align.len()])
            .max()
            .unwrap_or(0);
        let widths: Vec<usize> = (0..cols)
            .map(|c| {
                rows.iter()
                    .filter_map(|r| r.get(c))
                    .map(|s| s.width())
                    .max()
                    .unwrap_or(0)
                    .max(3)
            })
            .collect();
        let cell = |text: &str, c: usize| {
            let pad = widths[c] - text.width();
            match align.get(c).copied().unwrap_or('n') {
                'r' => format!("{}{text}", " ".repeat(pad)),
                'c' => format!("{}{text}{}", " ".repeat(pad / 2), " ".repeat(pad - pad / 2)),
                _ => format!("{text}{}", " ".repeat(pad)),
            }
        };
        let line = |cells: Vec<String>| format!("| {} |", cells.join(" | "));
        let delim: Vec<String> = (0..cols)
            .map(|c| {
                let w = widths[c];
                match align.get(c).copied().unwrap_or('n') {
                    'l' => format!(":{}", "-".repeat(w - 1)),
                    'r' => format!("{}:", "-".repeat(w - 1)),
                    'c' => format!(":{}:", "-".repeat(w - 2)),
                    _ => "-".repeat(w),
                }
            })
            .collect();
        let mut out = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            out.push(line(
                r.iter().enumerate().map(|(c, t)| cell(t, c)).collect(),
            ));
            if i == 0 {
                out.push(line(delim.clone()));
            }
        }
        out
    }

    /// A row with more cells than the header (an unescaped `|` in a code span splits a cell): the
    /// extra cells would make the aligned table another table, so it stays as written.
    pub(super) fn ragged(&self, n: &SyntaxNode) -> bool {
        let mut rows = n.children().filter(|r| r.kind() == k::TABLE_ROW);
        let head = rows.next().map_or(0, |r| cell_count(&r));
        rows.any(|r| cell_count(&r) > head)
    }

    /// The cells of a table row; an empty cell has no node, and the outer pipes are optional.
    pub(super) fn row(&self, r: &SyntaxNode) -> Vec<String> {
        let mut cells: Vec<Option<String>> = vec![None];
        let (mut first, mut last_pipe) = (true, false);
        for e in r.children_with_tokens() {
            match e {
                SyntaxElement::Token(t) if t.kind() == k::PIPE => {
                    if first {
                        cells.clear();
                    }
                    cells.push(None);
                    last_pipe = true;
                    first = false;
                }
                SyntaxElement::Node(c) if c.kind() == k::CELL => {
                    let text = self.inline_lines(&c).join(" ");
                    if let Some(slot) = cells.last_mut() {
                        *slot = Some(text);
                    }
                    last_pipe = false;
                    first = false;
                }
                _ => {}
            }
        }
        if last_pipe {
            cells.pop();
        }
        cells.into_iter().map(Option::unwrap_or_default).collect()
    }
}

/// The length of `Render::row` without building the cells.
fn cell_count(r: &SyntaxNode) -> usize {
    let (mut n, mut first, mut last_pipe) = (1, true, false);
    for e in r.children_with_tokens() {
        match e.kind() {
            k::PIPE => {
                if first {
                    n = 0;
                }
                n += 1;
                (first, last_pipe) = (false, true);
            }
            k::CELL => (first, last_pipe) = (false, false),
            _ => {}
        }
    }
    n - usize::from(last_pipe)
}
