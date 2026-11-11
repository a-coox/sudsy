//! Spacing of an arithmetic expression like shfmt: binary operators, `,`, `?` and `:` get one
//! blank on each side, unary and `++`/`--` operators and subscripts none.

const OPS: [&str; 34] = [
    "<<=", ">>=", "&&", "||", "<<", ">>", "<=", ">=", "==", "!=", "+=", "-=", "*=", "/=", "%=",
    "&=", "|=", "^=", "**", "++", "--", "+", "-", "*", "/", "%", "<", ">", "=", "&", "|", "^", "!",
    "~",
];

/// `inner` (the text between `((` and `))`) respaced; `for_c` also splits at `;`, `nested` respaces
/// a `$(( ))` inside (the tree holds it as one restyled token only when the whole is one). `None`: the
/// text holds something this does not read, so the caller keeps it as written.
pub fn respace(inner: &str, for_c: bool, nested: bool) -> Option<String> {
    let c: Vec<char> = inner.chars().collect();
    let mut at = 0;
    let out = expr(&c, &mut at, (for_c, nested))?;
    (at == c.len()).then_some(out)
}

/// The text up to the closing `)` or `]` (left unread), a `;` or the end.
fn expr(c: &[char], at: &mut usize, mode: (bool, bool)) -> Option<String> {
    let mut out = String::new();
    let mut operand = false;
    while let Some(&ch) = c.get(*at) {
        match ch {
            ' ' | '\t' => *at += 1,
            ')' | ']' => break,
            ';' if mode.0 => {
                if !out.ends_with("; ") {
                    out.truncate(out.trim_end().len());
                }
                out.push_str("; ");
                operand = false;
                *at += 1;
            }
            ',' | '?' | ':' => {
                bin(&mut out, &ch.to_string(), ch == ',');
                operand = false;
                *at += 1;
            }
            '(' => {
                *at += 1;
                out.push('(');
                out.push_str(&expr(c, at, (false, mode.1))?);
                (c.get(*at) == Some(&')')).then_some(())?;
                out.push(')');
                *at += 1;
                operand = true;
            }
            '"' | '\'' | '`' => {
                let end = (*at + 1..c.len()).find(|&i| c[i] == ch && c[i - 1] != '\\')?;
                out.extend(&c[*at..=end]);
                *at = end + 1;
                operand = true;
            }
            '$' => {
                out.push_str(&dollar(c, at, mode.1)?);
                operand = true;
            }
            _ if ch.is_alphanumeric() || matches!(ch, '_' | '#' | '@' | '.') => {
                while c
                    .get(*at)
                    .is_some_and(|&x| x.is_alphanumeric() || matches!(x, '_' | '#' | '@' | '.'))
                {
                    out.push(c[*at]);
                    *at += 1;
                }
                // shortcut: a subscript is kept as written (the tree holds it as one word), respace it when the tree can
                if c.get(*at) == Some(&'[') {
                    let end = (*at..c.len()).find(|&i| c[i] == ']')?;
                    out.extend(&c[*at..=end]);
                    *at = end + 1;
                }
                operand = true;
            }
            _ => {
                let op = OPS
                    .iter()
                    .find(|o| c[*at..].iter().take(3).collect::<String>().starts_with(**o))?;
                *at += op.len();
                match (*op, operand) {
                    ("++" | "--", true) => out.push_str(op),
                    ("!", _) => {
                        out.push_str("! ");
                        operand = false;
                    }
                    ("++" | "--" | "~", _) | ("+" | "-", false) => {
                        out.push_str(op);
                        operand = false;
                    }
                    _ => {
                        bin(&mut out, op, false);
                        operand = false;
                    }
                }
            }
        }
    }
    if !out.ends_with("; ") {
        out.truncate(out.trim_end().len());
    }
    Some(out)
}

/// ` op ` (`, ` without the blank before).
fn bin(out: &mut String, op: &str, tight: bool) {
    out.truncate(out.trim_end().len());
    if !tight {
        out.push(' ');
    }
    out.push_str(op);
    out.push(' ');
}

/// `$name`, `${...}`, `$(...)` (kept) or `$((...))` (respaced), from the `$`.
fn dollar(c: &[char], at: &mut usize, nested: bool) -> Option<String> {
    let start = *at;
    *at += 1;
    match c.get(*at) {
        Some('(') if nested && c.get(*at + 1) == Some(&'(') => {
            *at += 2;
            let inner = expr(c, at, (false, true))?;
            (c.get(*at..*at + 2)? == [')', ')']).then_some(())?;
            *at += 2;
            Some(format!("$(({inner}))"))
        }
        Some(&open @ ('(' | '{')) => {
            let close = if open == '(' { ')' } else { '}' };
            let mut depth = 0;
            for i in *at..c.len() {
                depth += i32::from(c[i] == open) - i32::from(c[i] == close);
                if depth == 0 {
                    *at = i + 1;
                    return Some(c[start..*at].iter().collect());
                }
            }
            None
        }
        _ => {
            if c.get(*at)
                .is_some_and(|x| matches!(x, '#' | '?' | '@' | '*' | '!' | '$' | '-'))
            {
                *at += 1;
            }
            while c.get(*at).is_some_and(|x| x.is_alphanumeric() || *x == '_') {
                *at += 1;
            }
            Some(c[start..*at].iter().collect())
        }
    }
}
