//! Spelling of single tokens: numbers, units, strings, hex colors and `url(`.

/// Units written in their canonical case (`PX` becomes `px`, `hz` becomes `Hz`).
const UNITS: &[&str] = &[
    "em", "rem", "ex", "rex", "cap", "rcap", "ch", "rch", "ic", "ric", "lh", "rlh", "vw", "svw",
    "lvw", "dvw", "vh", "svh", "lvh", "dvh", "vi", "svi", "lvi", "dvi", "vb", "svb", "lvb", "dvb",
    "vmin", "svmin", "lvmin", "dvmin", "vmax", "svmax", "lvmax", "dvmax", "cm", "mm", "Q", "in",
    "pt", "pc", "px", "cqw", "cqh", "cqi", "cqb", "cqmin", "cqmax", "deg", "grad", "rad", "turn",
    "s", "ms", "Hz", "kHz", "dpi", "dpcm", "dppx", "x", "fr",
];

/// A number token (or the numeric start of a dimension): lowercase `e`, no `+` or leading zeros in
/// the exponent, no trailing zeros or dot, a digit before the dot. Returns the text after the
/// number as the second element.
pub fn split_number(s: &str) -> (String, &str) {
    let b = s.as_bytes();
    let mut i = 0;
    let sign = if matches!(b.first(), Some(b'+' | b'-')) {
        i = 1;
        &s[..1]
    } else {
        ""
    };
    let digits = |from: usize| from + b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int_end = digits(i);
    let int = &s[i..int_end];
    i = int_end;
    let mut frac = "";
    if b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
        let end = digits(i + 1);
        frac = &s[i + 1..end];
        i = end;
    }
    let mut exp = String::new();
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let sign_len = usize::from(matches!(b.get(i + 1), Some(b'+' | b'-')));
        if b.get(i + 1 + sign_len).is_some_and(u8::is_ascii_digit) {
            let end = digits(i + 1 + sign_len);
            let neg = b[i + 1] == b'-';
            let n = s[i + 1 + sign_len..end].trim_start_matches('0');
            if !n.is_empty() {
                exp = format!("e{}{n}", if neg { "-" } else { "" });
            }
            i = end;
        }
    }
    let frac = frac.trim_end_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    let dot = if frac.is_empty() { "" } else { "." };
    (format!("{sign}{int}{dot}{frac}{exp}"), &s[i..])
}

pub fn number(s: &str) -> String {
    split_number(s).0
}

pub fn dimension(s: &str) -> String {
    let (n, unit) = split_number(s);
    let canonical = UNITS.iter().find(|u| u.eq_ignore_ascii_case(unit));
    format!("{n}{}", canonical.copied().unwrap_or(unit))
}

pub fn percentage(s: &str) -> String {
    format!("{}%", number(s.trim_end_matches('%')))
}

/// A string token with the preferred quote unless it holds more of that quote than of the other;
/// one that has its quote already is kept as written.
pub fn string(raw: &str, single_quote: bool) -> String {
    let body = &raw[1..raw.len() - 1];
    let (pref, alt) = if single_quote {
        ('\'', '"')
    } else {
        ('"', '\'')
    };
    let count = |q: char| body.matches(q).count();
    let (quote, other) = if count(pref) > count(alt) {
        (alt, pref)
    } else {
        (pref, alt)
    };
    if raw.starts_with(quote) {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    out.push(quote);
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(e) if e == other => out.push(e),
                Some(e) => {
                    out.push('\\');
                    out.push(e);
                }
                None => out.push('\\'),
            },
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// `#FFF` is `#fff`; other hashes (ids, odd lengths) keep their case.
pub fn hash(s: &str) -> String {
    let hex = &s[1..];
    if matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        s.to_ascii_lowercase()
    } else {
        s.to_string()
    }
}

/// `url( a.png )` is `url(a.png)`.
pub fn url(s: &str) -> String {
    match (s.find('('), s.strip_suffix(')')) {
        (Some(open), Some(body)) => format!("{}{})", &body[..=open], body[open + 1..].trim()),
        _ => s.to_string(),
    }
}

/// An identifier in a value: the CSS-wide keywords are lowercase.
pub fn keyword(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if matches!(lower.as_str(), "initial" | "inherit" | "unset" | "revert") {
        lower
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        for (a, b) in [
            (".50", "0.5"),
            ("0.0", "0"),
            ("+1E3", "+1e3"),
            ("1e+05", "1e5"),
            ("1e-0", "1"),
            ("-.5", "-0.5"),
        ] {
            assert_eq!(number(a), b, "{a}");
        }
        assert_eq!(dimension("1.0PX"), "1px");
        assert_eq!(dimension("1em"), "1em");
        assert_eq!(dimension("2Q"), "2Q");
    }

    #[test]
    fn strings() {
        assert_eq!(string("'it\\'s'", false), "\"it's\"");
        assert_eq!(string("'a\"b'", false), "'a\"b'");
        assert_eq!(string("\"a\"", true), "'a'");
        assert_eq!(string("\"a\\\"b\"", false), "'a\"b'");
        assert_eq!(string("\"\\\"\\'\"", false), "\"\\\"\\'\"");
    }
}
