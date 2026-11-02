//! Turns the parser's events into the lossless tree. The parser only reports real tokens; the
//! text between two of them is trivia (blanks, line breaks, comments), lexed here and split by
//! the shared attachment rule: the part up to and including the first line break is trailing
//! trivia of the previous token, the rest is leading trivia of the next one.

use crate::generated::kind as k;
use core_syntax::trivia::is_line_break;
use core_syntax::{SyntaxKind, SyntaxNode, TreeBuilder};
use provider_api::ParseError;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Ev {
    Start(SyntaxKind),
    Tok(SyntaxKind, usize, usize),
    Finish,
}

type Trivia = Vec<(SyntaxKind, usize, usize)>;

/// Blanks, line breaks and comments in `src[start..end]`. Anything else is reported as an
/// `Unknown` token (a parser bug, or text it chose to leave alone) so no byte is ever lost.
fn lex_gap(src: &str, start: usize, end: usize, errors: &mut Vec<ParseError>) -> Trivia {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = start;
    while i < end {
        let s = i;
        match b[i] {
            b' ' | b'\t' => {
                while i < end && matches!(b[i], b' ' | b'\t') {
                    i += 1;
                }
                out.push((k::WHITESPACE, s, i));
            }
            b'\n' => {
                i += 1;
                out.push((k::NEWLINE, s, i));
            }
            b'\r' => {
                i += 1;
                if i < end && b[i] == b'\n' {
                    i += 1;
                }
                out.push((k::NEWLINE, s, i));
            }
            b'#' => {
                while i < end && !matches!(b[i], b'\n' | b'\r') {
                    i += 1;
                }
                out.push((k::COMMENT, s, i));
            }
            _ if src[i..].starts_with('\u{feff}') => {
                i += 3;
                out.push((k::WHITESPACE, s, i));
            }
            _ => {
                while i < end && !matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
                    i += 1;
                }
                // Stay on a character boundary.
                while !src.is_char_boundary(i) {
                    i += 1;
                }
                errors.push(ParseError {
                    start: s as u32,
                    len: (i - s) as u32,
                    code: "unexpected-text".into(),
                    message: "unexpected text".into(),
                });
                out.push((k::UNKNOWN, s, i));
            }
        }
    }
    out
}

fn ends_line(src: &str, t: &(SyntaxKind, usize, usize)) -> bool {
    src[t.1..t.2].chars().next_back().is_some_and(is_line_break)
}

pub(crate) fn build(
    src: &str,
    events: &[Ev],
    mut errors: Vec<ParseError>,
) -> (SyntaxNode, Vec<ParseError>) {
    // Start of the next real token after each event index.
    let mut next_start = vec![src.len(); events.len() + 1];
    for (i, ev) in events.iter().enumerate().rev() {
        next_start[i] = match ev {
            Ev::Tok(_, s, _) => *s,
            _ => next_start[i + 1],
        };
    }
    let mut b = TreeBuilder::new();
    let mut pending: Trivia = Vec::new();
    let mut prev_end = 0;
    let mut first = true;
    for (i, ev) in events.iter().enumerate() {
        match *ev {
            Ev::Start(kind) => b.start_node(kind),
            Ev::Finish => b.finish_node(),
            Ev::Tok(kind, s, e) => {
                if first {
                    pending = lex_gap(src, 0, s, &mut errors);
                    first = false;
                }
                for (tk, ts, te) in pending.drain(..) {
                    b.token(tk, &src[ts..te]);
                }
                b.token(kind, &src[s..e]);
                prev_end = e;
                let gap = lex_gap(src, prev_end, next_start[i + 1], &mut errors);
                let cut = gap
                    .iter()
                    .position(|t| ends_line(src, t))
                    .map_or(gap.len(), |p| p + 1);
                let mut gap = gap.into_iter();
                for (tk, ts, te) in gap.by_ref().take(cut) {
                    b.token(tk, &src[ts..te]);
                }
                pending = gap.collect();
            }
        }
    }
    let _ = prev_end;
    (SyntaxNode::new_root(b.finish()), errors)
}
