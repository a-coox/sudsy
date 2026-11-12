//! What the safety check compares of a `Value` node (tagged `#restyle`): its tokens with the
//! text the formatter does not change, joined by one space, a unary sign attached to its operand.
//! The formatter joins `+ 20px` after a comma and splits `round(1) +2` into `+ 2`, which changes
//! the token count; a sign is unary or binary by its left neighbour, not by blanks, so `1px -2px`
//! (a list) and `1px - 2px` (a subtraction) stay different.

use super::{normalized, value::is_trivia};
use crate::generated::kind as k;
use core_syntax::{SyntaxKind, SyntaxNode};

pub(super) fn value(node: &SyntaxNode) -> String {
    let toks: Vec<_> = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !is_trivia(t.kind()))
        .collect();
    let (mut out, mut prev, mut glue) = (String::new(), None::<(SyntaxKind, bool)>, true);
    let mut push = |text: &str, glue_next: bool, glue: &mut bool| {
        if !out.is_empty() && !*glue {
            out.push(' ');
        }
        out.push_str(text);
        *glue = glue_next;
    };
    for (i, t) in toks.iter().enumerate() {
        let kind = t.kind();
        // A trailing comma or semicolon is the formatter's to add or remove.
        let ends = toks
            .get(i + 1)
            .is_none_or(|t| matches!(t.kind(), k::R_PAREN | k::R_BRACKET | k::R_BRACE));
        if matches!(kind, k::COMMA | k::SEMICOLON) && ends {
            continue;
        }
        // `50.` is `50`: a dot right after a number, alone or before `...`.
        let adjacent_dots = toks[i..]
            .windows(2)
            .take_while(|w| {
                w[0].text() == "."
                    && w[1].text() == "."
                    && w[1].text_range().start() == w[0].text_range().end()
            })
            .count();
        let run = if t.text() == "." {
            adjacent_dots + 1
        } else {
            0
        };
        if matches!(run, 1 | 4)
            && i > 0
            && matches!(toks[i - 1].kind(), k::NUMBER | k::DIMENSION | k::PERCENTAGE)
            && toks[i - 1].text_range().end() == t.text_range().start()
        {
            continue;
        }
        let text = normalized(t);
        let operator = |k2: SyntaxKind| k2 == k::DELIM;
        let after_operand = matches!(prev, Some((k::R_PAREN | k::VARIABLE | k::AT_KEYWORD, _)));
        if matches!(kind, k::NUMBER | k::DIMENSION | k::PERCENTAGE)
            && text.starts_with('+')
            && after_operand
        {
            push("+", false, &mut glue);
            push(&text[1..], false, &mut glue);
        } else if kind == k::DELIM && matches!(&*text, "+" | "-") {
            let unary = match prev {
                None => true,
                Some((p, _)) => {
                    operator(p) || matches!(p, k::COMMA | k::L_PAREN | k::L_BRACKET | k::COLON)
                }
            };
            push(&text, unary, &mut glue);
        } else {
            push(&text, false, &mut glue);
        }
        prev = Some((kind, false));
    }
    out
}
