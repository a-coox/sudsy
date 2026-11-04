//! `align_comments`: the trailing comments of consecutive entries start in one column. A run is
//! the entries of one table with no blank line or comment line between them; the column is one
//! past the widest entry of the run (entries without a comment count). A run with an entry on
//! several lines is not aligned.
//!
//! The widths come from formatting the entries once in a context of their own (only for runs
//! that have a comment); the formatter then pads the comment of each entry.

use crate::format::TomlFormatter;
use crate::generated::kind as k;
use core_format::comments;
use core_format::doc::{self, Doc};
use core_format::printer;
use core_format::{FormatCtx, print_options};
use core_syntax::SyntaxNode;
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

/// Extra spaces in front of the trailing comment of an entry, by the entry's start offset.
#[derive(Default)]
pub struct AlignTable(HashMap<u32, usize>);

impl AlignTable {
    pub fn extra(&self, entry: &SyntaxNode) -> usize {
        self.0
            .get(&u32::from(entry.text_range().start()))
            .copied()
            .unwrap_or(0)
    }

    /// The runs of the whole document.
    pub fn build(cx: &FormatCtx<'_>, root: &SyntaxNode) -> Self {
        let measure = FormatCtx::new(cx.grammar, cx.options, cx.directives, &TomlFormatter);
        let mut table = AlignTable::default();
        table.container(&measure, root);
        for t in root
            .children()
            .filter(|n| matches!(n.kind(), k::TABLE | k::ARRAY_TABLE))
        {
            table.container(&measure, &t);
        }
        table
    }

    fn container(&mut self, cx: &FormatCtx<'_>, parent: &SyntaxNode) {
        let mut run: Vec<SyntaxNode> = Vec::new();
        for pair in parent.children().filter(|n| n.kind() == k::PAIR) {
            let apart = cx.first_token(&pair).is_some_and(|t| {
                !comments::leading_comments(cx.grammar, &t).is_empty()
                    || comments::blank_lines_before(cx.grammar, &t) > 0
            });
            if apart {
                self.run(cx, &std::mem::take(&mut run));
            }
            run.push(pair);
        }
        self.run(cx, &run);
    }

    /// Measures the entries of a run and pads those with a comment. A run with an entry on
    /// several lines is left alone.
    fn run(&mut self, cx: &FormatCtx<'_>, run: &[SyntaxNode]) {
        if !run.iter().any(|p| trailing(cx, p).is_some()) {
            return;
        }
        let Some(widths) = run.iter().map(|p| width(cx, p)).collect::<Option<Vec<_>>>() else {
            return;
        };
        let widest = widths.iter().copied().max().unwrap_or(0);
        for (pair, w) in run.iter().zip(widths) {
            if trailing(cx, pair).is_some() && widest > w {
                self.0
                    .insert(u32::from(pair.text_range().start()), widest - w);
            }
        }
    }
}

fn trailing(cx: &FormatCtx<'_>, pair: &SyntaxNode) -> Option<String> {
    let last = cx.last_token(pair)?;
    comments::trailing_comments(cx.grammar, &last)
        .last()
        .map(|c| c.text().into_owned())
}

/// Display width of the entry without its comment; `None` when it takes several lines.
fn width(cx: &FormatCtx<'_>, pair: &SyntaxNode) -> Option<usize> {
    let mut d = cx.format(pair).ok()?;
    let opts = print_options(cx.options);
    let printed = printer::print(&mut d, &opts);
    // The comment lines in front of the entry are not part of its width.
    let lead = cx
        .first_token(pair)
        .map(|t| printer::print(&mut cx.leading_comments_doc(&t), &opts))
        .unwrap_or_default();
    let line = printed
        .strip_prefix(lead.as_str())
        .unwrap_or(&printed)
        .trim_end();
    if line.contains('\n') {
        return None;
    }
    let code = match trailing(cx, pair) {
        Some(c) => line.strip_suffix(c.as_str()).unwrap_or(line),
        None => line,
    };
    Some(code.trim_end().width())
}

/// Puts `extra` spaces in front of the last trailing comment in `d`.
pub fn pad_comment(d: &mut Doc, extra: usize) -> bool {
    match d {
        Doc::LineSuffix(body) => {
            let inner = std::mem::replace(&mut **body, Doc::Nil);
            **body = doc::concat(vec![doc::text(" ".repeat(extra)), inner]);
            true
        }
        Doc::Concat(parts) => parts.iter_mut().rev().any(|p| pad_comment(p, extra)),
        Doc::Group(g) => pad_comment(&mut g.contents, extra),
        Doc::Indent(inner) | Doc::Align(_, inner) => pad_comment(inner, extra),
        _ => false,
    }
}
