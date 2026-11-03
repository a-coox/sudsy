//! The value of a JSON string token.

use std::borrow::Cow;

/// The text of a string token (quotes included) with its escapes decoded. A lone surrogate
/// becomes U+FFFD; an escape the parser rejects is kept as written.
pub fn string_value(token: &str) -> Cow<'_, str> {
    let inner = token.get(1..token.len().saturating_sub(1)).unwrap_or(token);
    if !inner.contains('\\') {
        return Cow::Borrowed(inner);
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('b') => out.push('\u{8}'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let unit = hex4(&mut chars);
                let ch = match unit {
                    Some(hi @ 0xD800..=0xDBFF) => {
                        let mut ahead = chars.clone();
                        let low = (ahead.next() == Some('\\') && ahead.next() == Some('u'))
                            .then(|| hex4(&mut ahead))
                            .flatten()
                            .filter(|l| (0xDC00..=0xDFFF).contains(l));
                        match low {
                            Some(lo) => {
                                chars = ahead;
                                char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00))
                            }
                            None => None,
                        }
                    }
                    Some(u) => char::from_u32(u),
                    None => None,
                };
                out.push(ch.unwrap_or('\u{FFFD}'));
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    Cow::Owned(out)
}

fn hex4(chars: &mut impl Iterator<Item = char>) -> Option<u32> {
    let mut v = 0;
    for _ in 0..4 {
        v = v * 16 + chars.next()?.to_digit(16)?;
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::string_value;

    #[test]
    fn escapes_decode() {
        assert_eq!(string_value(r#""a""#), "a");
        assert_eq!(string_value(r#""a\/\n""#), "a/\n");
        assert_eq!(string_value(r#""😀""#), "\u{1F600}");
        assert_eq!(string_value(r#""\ud83d""#), "\u{FFFD}");
    }
}
