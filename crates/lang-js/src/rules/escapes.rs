//! Rules about the text of literals: escapes, regex source, whitespace, number precision.

use super::{Check, code_text, code_token, span};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_syntax::{SyntaxNode, TextRange, TextSize};

fn at(start: TextSize, offset: usize, len: usize) -> TextRange {
    TextRange::at(
        start + TextSize::from(offset as u32),
        TextSize::from(len as u32),
    )
}

/// Calls `found(byte offset, escaped char)` for each `\x` of a string or template chunk that
/// escapes nothing special.
fn string_escapes(content: &str, template: bool, quote: char, mut found: impl FnMut(usize, char)) {
    let chars: Vec<(usize, char)> = content.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (pos, ch) = chars[i];
        let Some(&(after, next)) = chars.get(i + 1).filter(|_| ch == '\\') else {
            i += 1;
            continue;
        };
        if next.is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut unnecessary = !matches!(
            next,
            '\\' | 'n'
                | 'r'
                | 'v'
                | 't'
                | 'b'
                | 'f'
                | 'u'
                | 'x'
                | '\n'
                | '\r'
                | '\u{2028}'
                | '\u{2029}'
        );
        let is_quote = next == if template { '`' } else { quote };
        if template && next == '$' {
            unnecessary = !content[after + 1..].starts_with('{');
        } else if template && next == '{' {
            unnecessary = !content[..pos].ends_with('$');
        }
        if unnecessary && !is_quote {
            found(pos, next);
        }
        i += 2;
    }
}

const GENERAL: &str = "\\bcdDfnpPrsStvwWxu0123456789]";
const OUTSIDE_CLASS: &str = "^/.$*+?[{}|()Bk";

/// The escapes of a regex body that escape nothing special, outside and inside classes.
fn regex_escapes(body: &str, mut found: impl FnMut(usize, char)) {
    let chars: Vec<(usize, char)> = body.char_indices().collect();
    let (mut i, mut class) = (0, None::<usize>);
    while i < chars.len() {
        let (pos, ch) = chars[i];
        if ch == '\\' {
            let Some(&(_, next)) = chars.get(i + 1) else {
                break;
            };
            let allowed = match class {
                Some(start) if next == '^' && i == start + 1 => true,
                Some(start)
                    if next == '-'
                        && i != start + 1
                        && chars.get(i + 2).is_none_or(|c| c.1 != ']') =>
                {
                    true
                }
                Some(_) => GENERAL.contains(next),
                None => GENERAL.contains(next) || OUTSIDE_CLASS.contains(next),
            };
            if !allowed {
                found(pos, next);
            }
            i += 2;
            continue;
        }
        match (ch, class) {
            ('[', None) => class = Some(i),
            (']', Some(_)) => class = None,
            _ => {}
        }
        i += 1;
    }
}

/// The body of a regex literal token text (`/body/flags`).
fn regex_body(text: &str) -> &str {
    &text[1..text.rfind('/').unwrap_or(text.len()).max(1)]
}

pub(super) fn useless_escape(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let parent = n.parent();
    let report = |ctx: &mut RuleContext<'_>, start: TextSize, pos: usize, ch: char| {
        c.hit(ctx, at(start, pos, 1 + ch.len_utf8()), &ch.to_string());
    };
    match n.kind() {
        k::STRING_LITERAL => {
            if parent.is_some_and(|p| p.kind() == k::JSX_ATTRIBUTE) {
                return;
            }
            let Some(token) = code_token(n, ctx.file.grammar) else {
                return;
            };
            let text = token.text();
            let Some(quote) = text.chars().next() else {
                return;
            };
            let start = token.text_range().start() + TextSize::from(1);
            let inner = &text[1..text.len().saturating_sub(1).max(1)];
            string_escapes(inner, false, quote, |pos, ch| report(ctx, start, pos, ch));
        }
        k::TEMPLATE_LITERAL => {
            let tagged = parent.is_some_and(|p| {
                p.kind() == k::TAGGED_TEMPLATE_EXPRESSION && p.children().last().as_ref() == Some(n)
            });
            if tagged {
                return;
            }
            for token in n
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| t.kind() == k::TEMPLATE_CHUNK)
            {
                let text = token.text();
                let from = 1;
                let to = if text.ends_with("${") {
                    text.len() - 2
                } else {
                    text.len() - 1
                };
                if to > from {
                    let start = token.text_range().start() + TextSize::from(from as u32);
                    string_escapes(&text[from..to], true, '`', |pos, ch| {
                        report(ctx, start, pos, ch)
                    });
                }
            }
        }
        _ => {
            let Some(token) = code_token(n, ctx.file.grammar) else {
                return;
            };
            let start = token.text_range().start() + TextSize::from(1);
            regex_escapes(regex_body(token.text()), |pos, ch| {
                report(ctx, start, pos, ch)
            });
        }
    }
}

/// Runs of two or more spaces outside character classes, as the length to report: a run followed by a
/// quantifier gives its last space to it.
fn space_runs(pattern: &str) -> Vec<usize> {
    let chars: Vec<char> = pattern.chars().collect();
    let (mut runs, mut i, mut class) = (Vec::new(), 0, false);
    while i < chars.len() {
        match chars[i] {
            '\\' if chars.get(i + 1) != Some(&' ') => i += 1,
            '[' => class = true,
            ']' => class = false,
            ' ' if !class => {
                let len = chars[i..].iter().take_while(|c| **c == ' ').count();
                let quantified = chars.get(i + len).is_some_and(|c| "+*{?".contains(*c));
                if len >= 2 && !quantified {
                    runs.push(len);
                } else if len >= 3 {
                    runs.push(len - 1);
                }
                i += len - 1;
            }
            _ => {}
        }
        i += 1;
    }
    runs
}

/// `/a  b/` and `new RegExp("a  b")`.
pub(super) fn regex_spaces(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let pattern = if n.kind() == k::REG_EXP_LITERAL {
        code_token(n, ctx.file.grammar).map(|t| regex_body(t.text()).to_string())
    } else {
        let mut parts = n.children();
        let callee = parts
            .next()
            .filter(|f| f.kind() == k::IDENTIFIER_REFERENCE && code_text(f, g) == "RegExp");
        let arg = parts.next().filter(|a| a.kind() == k::STRING_LITERAL);
        callee.and(arg).map(|a| {
            let text = code_text(&a, g);
            text[1..text.len() - 1].to_string()
        })
    };
    for len in pattern.iter().flat_map(|p| space_runs(p)) {
        c.hit(ctx, span(ctx, n), &len.to_string());
    }
}

/// `/[]/`
pub(super) fn empty_character_class(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(token) = code_token(n, ctx.file.grammar) else {
        return;
    };
    let chars: Vec<char> = regex_body(token.text()).chars().collect();
    let (mut i, mut class) = (0, false);
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            '[' if !class => {
                if chars.get(i + 1) == Some(&']') {
                    c.hit(ctx, span(ctx, n), "");
                    return;
                }
                class = true;
            }
            ']' => class = false,
            _ => {}
        }
        i += 1;
    }
}

fn irregular(ch: char) -> bool {
    matches!(
        ch,
        '\u{b}' | '\u{c}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{180e}' | '\u{2000}'
            ..='\u{200b}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// Whitespace other than space, tab and line breaks, outside strings (a BOM at the start is not
/// part of the text ESLint reads).
pub(super) fn irregular_whitespace(c: &Check, root: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let text = ctx.file.text;
    if text.bytes().all(|b| b.is_ascii() && b != 0x0b && b != 0x0c) {
        return;
    }
    let strings: Vec<TextRange> = (root.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::STRING)
        .map(|t| t.text_range())
        .collect();
    let mut run: Option<usize> = None;
    for (i, ch) in text
        .char_indices()
        .chain(std::iter::once((text.len(), 'a')))
    {
        match (run, irregular(ch) && !(i == 0 && ch == '\u{feff}')) {
            (None, true) => run = Some(i),
            (Some(start), false) => {
                run = None;
                let range = at(TextSize::from(0), start, i - start);
                if !strings.iter().any(|s| s.contains(range.start())) {
                    c.hit(ctx, range, "");
                }
            }
            _ => {}
        }
    }
}

/// Does the number literal `raw` not read back as itself?
fn loses_precision(raw: &str) -> bool {
    let s = raw.replace('_', "").to_ascii_lowercase();
    let legacy_octal =
        s.len() > 1 && s.starts_with('0') && s.bytes().all(|b| (b'0'..=b'7').contains(&b));
    let (radix, digits) = match (s.get(..2), s.as_str()) {
        (Some("0x"), _) => (16, &s[2..]),
        (Some("0o"), _) => (8, &s[2..]),
        (Some("0b"), _) => (2, &s[2..]),
        (_, _) if legacy_octal => (8, &s[1..]),
        _ => (10, s.as_str()),
    };
    if radix != 10 {
        // the value is exact when its significant bits (up to the last 1) fit in 53
        let bits: String = digits
            .chars()
            .map(|d| {
                format!(
                    "{:0w$b}",
                    d.to_digit(radix).unwrap_or(0),
                    w = radix.ilog2() as usize
                )
            })
            .collect();
        let bits = bits.trim_start_matches('0');
        return bits.len() > 1024 || bits.trim_end_matches('0').len() > 53;
    }
    let (mantissa, _) = s.split_once('e').unwrap_or((&s, ""));
    let significant = mantissa.replace('.', "");
    let significant = significant.trim_start_matches('0').trim_end_matches('0');
    if significant.is_empty() {
        return false;
    }
    let Ok(value) = s.parse::<f64>() else {
        return false;
    };
    if !value.is_finite() {
        return true;
    }
    let formatted = format!("{:.*e}", significant.len() - 1, value);
    let digits = formatted
        .split_once('e')
        .map_or("", |m| m.0)
        .replace('.', "");
    digits.trim_end_matches('0') != significant
}

pub(super) fn loss_of_precision(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if loses_precision(&code_text(n, ctx.file.grammar)) {
        c.hit(ctx, span(ctx, n), "");
    }
}
