//! Regexes of rules, compiled when first used and for ASCII text first.

use regex::{Regex, bytes};
use regex_syntax::ParserBuilder;
use regex_syntax::hir::{Hir, HirKind};
use std::sync::OnceLock;

/// The longest text checked for ASCII (and so matched by the ASCII form).
const ASCII_LIMIT: usize = 1024;

/// A regex that costs little until it matches something. Unicode classes (`\s`, `\w`, `.`,
/// `[^x]`) build large automata, and on ASCII text they match what their ASCII forms match, so
/// ASCII text goes to a regex compiled without Unicode (a fifth of the cost); other text, and
/// a pattern that needs Unicode (`\p{L}`), to the full regex. The answers are the same.
#[derive(Debug)]
pub struct Re {
    source: String,
    /// `None` inside: the pattern needs Unicode.
    ascii: OnceLock<Option<bytes::Regex>>,
    full: OnceLock<Regex>,
}

impl Re {
    /// Compiles the ASCII form now (or the full one, when the pattern needs Unicode), which
    /// checks the syntax. A counted repetition of more than a literal (`\w{n,m}`) is the way to
    /// multiply a Unicode class past the regex size limit, so such a pattern is built in full
    /// too, and an error here says it is too large.
    // shortcut: any other pattern is not built in full here, so one that is still too large
    // panics on first use on non-ASCII text; build every pattern in full if one is ever seen.
    pub fn new(source: &str) -> Result<Re, String> {
        let re = Self::unbuilt(source);
        if re.ascii().is_none() || counts_more_than_a_literal(source) {
            let full = Regex::new(source).map_err(|e| e.to_string())?;
            re.full.get_or_init(|| full);
        }
        Ok(re)
    }

    /// Checks the syntax only; the automata are built on the first [`Re::is_match`]. For regexes
    /// that may never be used.
    pub fn lazy(source: &str) -> Result<Re, String> {
        let re = Self::unbuilt(source);
        // A pattern valid without Unicode is valid with it; this skips the Unicode class tables.
        let plain = ParserBuilder::new()
            .unicode(false)
            .utf8(false)
            .build()
            .parse(source);
        if plain.is_err() {
            Regex::new(source).map_err(|e| e.to_string())?;
        }
        Ok(re)
    }

    fn unbuilt(source: &str) -> Re {
        Re {
            source: source.into(),
            ascii: OnceLock::new(),
            full: OnceLock::new(),
        }
    }

    fn ascii(&self) -> Option<&bytes::Regex> {
        self.ascii
            .get_or_init(|| {
                bytes::RegexBuilder::new(&self.source)
                    .unicode(false)
                    .build()
                    .ok()
            })
            .as_ref()
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.is_match_in(text, false)
    }

    /// [`Re::is_match`] on text that is a part of a file known to be ASCII (`in_ascii_file`), so
    /// that it need not be scanned.
    pub fn is_match_in(&self, text: &str, in_ascii_file: bool) -> bool {
        // Long text goes to the full regex: scanning it for ASCII would cost more than an
        // anchored match.
        if (in_ascii_file || text.len() <= ASCII_LIMIT && text.is_ascii())
            && let Some(re) = self.ascii()
        {
            return re.is_match(text.as_bytes());
        }
        self.full
            .get_or_init(|| Regex::new(&self.source).expect("the syntax was checked"))
            .is_match(text)
    }
}

/// Whether the pattern repeats anything but a literal a counted number of times (`{n}`,
/// `{n,}`, `{n,m}` with a count over one).
fn counts_more_than_a_literal(source: &str) -> bool {
    fn walk(hir: &Hir) -> bool {
        match hir.kind() {
            HirKind::Repetition(r) => {
                let counted = r.min > 1 || r.max.is_some_and(|m| m > 1);
                (counted && !matches!(r.sub.kind(), HirKind::Literal(_))) || walk(&r.sub)
            }
            HirKind::Capture(c) => walk(&c.sub),
            HirKind::Concat(hs) | HirKind::Alternation(hs) => hs.iter().any(walk),
            _ => false,
        }
    }
    ParserBuilder::new()
        .unicode(false)
        .utf8(false)
        .build()
        .parse(source)
        .is_ok_and(|hir| walk(&hir))
}

#[cfg(test)]
mod tests {
    use super::Re;

    /// Both paths give what `regex::Regex` gives, on ASCII and other text, for patterns that
    /// compile without Unicode and ones that need it.
    #[test]
    fn agrees_with_the_unicode_regex() {
        let patterns = [
            r"^\s*\w+\s*=",
            r"\S+\b",
            r"^[^=]*\bconst\b",
            r"(?i)^k.s$",
            r"^.{3}$",
            r"\p{L}+",
            r"[é。]",
        ];
        let texts = [
            "a = 1",
            "  \u{a0}x=",
            "const x",
            "K\u{212a}s",
            "é。é",
            "日本語",
            "",
            "ab\n",
        ];
        for pattern in patterns {
            let re = Re::new(pattern).unwrap();
            let lazy = Re::lazy(pattern).unwrap();
            let full = regex::Regex::new(pattern).unwrap();
            for text in texts {
                let want = full.is_match(text);
                assert_eq!(re.is_match(text), want, "{pattern} on {text:?}");
                assert_eq!(lazy.is_match(text), want, "lazy {pattern} on {text:?}");
            }
        }
        assert!(Re::new("(").is_err() && Re::lazy("(").is_err());
    }

    /// A pattern whose Unicode form is over the size limit is an error from `new`, not a panic
    /// on non-ASCII text.
    #[test]
    fn a_pattern_too_large_for_unicode_is_an_error() {
        assert!(Re::new(r"\w{1000}\w{1000}\w{1000}").is_err());
        assert!(Re::new(r"\w{3}").is_ok());
    }
}
