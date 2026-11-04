//! Shared comment rules, written against the grammar's comment syntax: `comment-space` and
//! `no-empty-comment`. They read comment tokens, so they cost nothing when off.

use crate::diagnostic::{Edit, Fix, FixKind};
use crate::rule;
use crate::rule::{FileContext, RuleContext};
use crate::tree::{is_code, structured_trivia};
use core_syntax::{Grammar, SyntaxToken, TextRange};

/// The comment tokens of the file (line and block comments, and documentation exteriors).
fn comments<'a>(file: &'a FileContext<'a>) -> impl Iterator<Item = SyntaxToken> + 'a {
    file.root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| file.grammar.has_tag(t.kind(), "comment"))
}

rule! {
    pub CommentSpace {
        id: "comment-space",
        category: Style,
        default: Off,
        fix: Safe,
        description: "A line comment has text right after its marker. Exempt: nothing but \
                  whitespace after the marker, a marker repeated (`////`, `##`), `--` separator \
                  lines, and a `#!` shebang on the first line. The fix inserts the space.",
        example: "//comment   // write: // comment",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(marker) = file.grammar.comments.line.as_deref() else {
            return;
        };
        if !file.text.contains(marker) {
            return;
        }
        let repeated = marker.chars().last().unwrap_or(' ');
        for token in comments(file) {
            let Some(rest) = token.text().strip_prefix(marker) else {
                continue;
            };
            let start = token.text_range().start();
            let exempt = rest.trim().is_empty()
                || rest.starts_with([' ', repeated])
                || rest.starts_with("--")
                || (usize::from(start) == 0 && rest.starts_with('!'));
            if exempt {
                continue;
            }
            let after = start + core_syntax::TextSize::of(marker);
            ctx.report(
                TextRange::new(start, after),
                format!("a comment should start with a space after `{marker}`"),
            )
            .fix(Fix::new(
                "Insert a space",
                FixKind::Safe,
                vec![Edit::insert(after, " ")],
            ));
        }
    }
}

/// Does `token` follow code on its own line, which makes it part of that code's trailing trivia?
fn after_code_on_line(token: &SyntaxToken, grammar: &Grammar) -> bool {
    let mut t = token.prev_token();
    while let Some(p) = t {
        if grammar.has_tag(p.kind(), "whitespace") {
            t = p.prev_token();
        } else {
            return is_code(&p, grammar);
        }
    }
    false
}

/// Does the comment `token` have another comment, directive or disabled text before it
/// (`forward` false) or after it (true) in the same run of trivia? A run is a token's trailing
/// trivia (up to the end of its line) or the leading trivia that follows.
fn has_neighbour(token: &SyntaxToken, grammar: &Grammar, forward: bool) -> bool {
    let trailing = after_code_on_line(token, grammar);
    let mut crossed = false;
    let mut t = if forward {
        token.next_token()
    } else {
        token.prev_token()
    };
    while let Some(n) = t {
        if grammar.has_tag(n.kind(), "whitespace") {
        } else if grammar.has_tag(n.kind(), "newline") {
            if forward && trailing {
                return false;
            }
            crossed = true;
        } else if is_code(&n, grammar) {
            return false;
        } else if forward || !crossed {
            return true;
        } else {
            // Before the comment, across a line break: the same run unless that trivia is the
            // trailing trivia of the code before it.
            let first = structured_trivia(&n, grammar)
                .and_then(|s| s.first_token())
                .unwrap_or(n);
            return !after_code_on_line(&first, grammar);
        }
        t = if forward {
            n.next_token()
        } else {
            n.prev_token()
        };
    }
    false
}

rule! {
    pub NoEmptyComment {
        id: "no-empty-comment",
        category: Style,
        default: Off,
        description: "A comment has no text. A block comment is reported wherever it is; an empty line \
                  comment only when it begins or ends a run of comments, because a bare `//` \
                  between two comment lines separates paragraphs.",
        example: "// ----\n//\nint x;   // the empty comment above is reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let comment = &file.grammar.comments;
        for token in comments(file) {
            let text = token.text();
            let (empty, block) = match (comment.line.as_deref(), &comment.block) {
                (Some(line), _) if text.starts_with(line) => {
                    (text[line.len()..].trim().is_empty(), false)
                }
                (_, Some((open, close)))
                    if text.len() >= open.len() + close.len()
                        && text.starts_with(open.as_str())
                        && text.ends_with(close.as_str()) =>
                {
                    (
                        text[open.len()..text.len() - close.len()].trim().is_empty(),
                        true,
                    )
                }
                _ => continue,
            };
            if !empty {
                continue;
            }
            if block
                || !(has_neighbour(&token, file.grammar, false)
                    && has_neighbour(&token, file.grammar, true))
            {
                ctx.report(token.text_range(), "the comment has no text");
            }
        }
    }
}
