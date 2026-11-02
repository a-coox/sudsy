//! Scalar values: line folding, escapes and block scalar semantics.

/// Splits at line breaks (`\n`, `\r\n`, `\r`).
fn split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\n' => {
                lines.push(&text[start..i]);
                i += 1;
                start = i;
            }
            b'\r' => {
                lines.push(&text[start..i]);
                i += if b.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
                start = i;
            }
            _ => i += 1,
        }
    }
    lines.push(&text[start..]);
    lines
}

/// Folds the lines of a flow scalar: surrounding blanks of every line are dropped, a single line
/// break becomes a space and `n` empty lines become `n` line feeds.
fn fold_flow(text: &str) -> String {
    let lines = split_lines(text);
    let last = lines.len() - 1;
    let mut out = String::new();
    let mut blanks = 0;
    for (i, line) in lines.iter().enumerate() {
        let mut line: &str = line;
        if i > 0 {
            line = line.trim_start_matches([' ', '\t']);
        }
        if i < last {
            line = line.trim_end_matches([' ', '\t']);
        }
        if line.is_empty() && i > 0 && i < last {
            blanks += 1;
            continue;
        }
        if i > 0 {
            if blanks == 0 {
                out.push(' ');
            } else {
                out.extend(std::iter::repeat_n('\n', blanks));
            }
        }
        blanks = 0;
        out.push_str(line);
    }
    out
}

pub fn plain_value(text: &str) -> String {
    fold_flow(text)
}

/// The value of a single-quoted scalar token (with its quotes).
pub fn single_quoted_value(token: &str) -> String {
    let inner = &token[1..token.len() - 1];
    fold_flow(inner).replace("''", "'")
}

/// The value of a double-quoted scalar token (with its quotes); `None` if an escape is invalid.
pub fn double_quoted_value(token: &str) -> Option<String> {
    let inner = &token[1..token.len() - 1];
    let chars: Vec<char> = inner.chars().collect();
    let mut out = String::new();
    // Output length that line folding must not trim (escapes and text before them).
    let mut protected = 0;
    let mut i = 0;
    let is_blank = |c: char| c == ' ' || c == '\t';
    let is_break = |c: char| c == '\n' || c == '\r';
    // Skips one line break at `i`; returns the new index.
    let skip_break = |chars: &[char], i: usize| {
        if chars[i] == '\r' && chars.get(i + 1) == Some(&'\n') {
            i + 2
        } else {
            i + 1
        }
    };
    // Skips blanks, then empty lines with their blanks; returns the index of the next content,
    // the number of empty lines passed.
    let skip_following = |chars: &[char], mut i: usize| {
        let mut empty = 0;
        loop {
            while i < chars.len() && is_blank(chars[i]) {
                i += 1;
            }
            if i < chars.len() && is_break(chars[i]) {
                i = skip_break(chars, i);
                empty += 1;
            } else {
                return (i, empty);
            }
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let e = *chars.get(i + 1)?;
            i += 2;
            let push = |out: &mut String, s: char| out.push(s);
            match e {
                '0' => push(&mut out, '\0'),
                'a' => push(&mut out, '\u{7}'),
                'b' => push(&mut out, '\u{8}'),
                't' | '\t' => push(&mut out, '\t'),
                'n' => push(&mut out, '\n'),
                'v' => push(&mut out, '\u{b}'),
                'f' => push(&mut out, '\u{c}'),
                'r' => push(&mut out, '\r'),
                'e' => push(&mut out, '\u{1b}'),
                ' ' => push(&mut out, ' '),
                '"' => push(&mut out, '"'),
                '/' => push(&mut out, '/'),
                '\\' => push(&mut out, '\\'),
                'N' => push(&mut out, '\u{85}'),
                '_' => push(&mut out, '\u{a0}'),
                'L' => push(&mut out, '\u{2028}'),
                'P' => push(&mut out, '\u{2029}'),
                'x' | 'u' | 'U' => {
                    let n = match e {
                        'x' => 2,
                        'u' => 4,
                        _ => 8,
                    };
                    let hex: String = chars.get(i..i + n)?.iter().collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    i += n;
                }
                '\n' | '\r' => {
                    // Escaped line break: no space; empty lines after it are line feeds.
                    i = skip_break(&chars, i - 1);
                    let (next, empty) = skip_following(&chars, i);
                    out.extend(std::iter::repeat_n('\n', empty));
                    i = next;
                }
                _ => return None,
            }
            protected = out.len();
        } else if is_break(c) {
            // Trim blanks before the break, then fold.
            while out.len() > protected && out.ends_with([' ', '\t']) {
                out.pop();
            }
            i = skip_break(&chars, i);
            let (next, empty) = skip_following(&chars, i);
            if empty == 0 {
                out.push(' ');
            } else {
                out.extend(std::iter::repeat_n('\n', empty));
            }
            i = next;
            protected = out.len();
        } else {
            out.push(c);
            i += 1;
        }
    }
    Some(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chomp {
    Strip,
    Clip,
    Keep,
}

/// A parsed block scalar header.
#[derive(Clone, Copy, Debug)]
pub struct BlockHeader {
    pub folded: bool,
    pub chomp: Chomp,
    pub indent: Option<usize>,
}

pub fn block_header(text: &str) -> BlockHeader {
    let mut h = BlockHeader {
        folded: text.starts_with('>'),
        chomp: Chomp::Clip,
        indent: None,
    };
    for c in text.chars().skip(1) {
        match c {
            '+' => h.chomp = Chomp::Keep,
            '-' => h.chomp = Chomp::Strip,
            '1'..='9' => h.indent = Some(c as usize - '0' as usize),
            _ => {}
        }
    }
    h
}

/// The value of a block scalar. `content` is the content token (indentation included) and `n`
/// the indentation of the parent node (-1 at the document level). The end of the input counts as
/// a line break.
pub fn block_value(header: BlockHeader, content: &str, n: i64) -> String {
    if content.is_empty() {
        return String::new();
    }
    // Lines, each with whether a line break ended it.
    let mut lines = split_lines(content);
    let ends_with_break = content.ends_with(['\n', '\r']);
    if ends_with_break {
        lines.pop();
    }
    let indent = match header.indent {
        Some(d) => (n + d as i64).max(0) as usize,
        // Without a line of text every line is blank, whatever its spaces.
        None => lines
            .iter()
            .find(|l| l.bytes().any(|c| c != b' '))
            .map_or(usize::MAX, |l| l.bytes().take_while(|c| *c == b' ').count()),
    };
    let stripped: Vec<&str> = lines
        .iter()
        .map(|l| {
            let spaces = l.bytes().take_while(|c| *c == b' ').count();
            &l[spaces.min(indent)..]
        })
        .collect();
    let last_text = stripped.iter().rposition(|l| !l.is_empty());
    let trailing_blank = stripped.len() - last_text.map_or(0, |i| i + 1);
    let mut body = String::new();
    if let Some(last) = last_text {
        if header.folded {
            let mut prev_spaced = false;
            let mut blanks = 0;
            let mut first = true;
            for line in &stripped[..=last] {
                if line.is_empty() {
                    blanks += 1;
                    continue;
                }
                let spaced = line.starts_with([' ', '\t']);
                if first {
                    body.extend(std::iter::repeat_n('\n', blanks));
                } else if !prev_spaced && !spaced {
                    if blanks == 0 {
                        body.push(' ');
                    } else {
                        body.extend(std::iter::repeat_n('\n', blanks));
                    }
                } else {
                    body.extend(std::iter::repeat_n('\n', blanks + 1));
                }
                body.push_str(line);
                prev_spaced = spaced;
                blanks = 0;
                first = false;
            }
        } else {
            body = stripped[..=last].join("\n");
        }
    }
    let final_break = true;
    match header.chomp {
        Chomp::Strip => {}
        Chomp::Clip => {
            if last_text.is_some() && final_break {
                body.push('\n');
            }
        }
        Chomp::Keep => {
            if last_text.is_some() && final_break {
                body.push('\n');
            }
            body.extend(std::iter::repeat_n('\n', trailing_blank));
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(header: &str, content: &str, n: i64) -> String {
        block_value(block_header(header), content, n)
    }

    #[test]
    fn flow_scalars_fold_lines() {
        assert_eq!(plain_value("a\n  b\n\n  c"), "a b\nc");
        assert_eq!(single_quoted_value("'it''s\n  ok'"), "it's ok");
        assert_eq!(double_quoted_value("\"a \\\n   b\"").unwrap(), "a b");
        assert_eq!(
            double_quoted_value("\"a\\x41\\u00e9\\n\"").unwrap(),
            "aAé\n"
        );
        assert_eq!(double_quoted_value("\"a  \n  b\"").unwrap(), "a b");
        assert!(double_quoted_value("\"\\q\"").is_none());
    }

    #[test]
    fn literal_scalars_follow_the_chomping_indicator() {
        assert_eq!(block("|", "  a\n  b", 0), "a\nb\n");
        assert_eq!(block("|-", "  a\n  b", 0), "a\nb");
        assert_eq!(block("|+", "  a\n\n\n", 0), "a\n\n\n");
        assert_eq!(block("|2", "   a", 0), " a\n");
    }

    #[test]
    fn folded_scalars_keep_more_indented_lines() {
        assert_eq!(block(">", "  a\n  b\n\n  c", 0), "a b\nc\n");
        assert_eq!(block(">", "  a\n   b\n  c", 0), "a\n b\nc\n");
    }
}
