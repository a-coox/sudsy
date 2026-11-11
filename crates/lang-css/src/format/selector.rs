//! Selector lists. A selector with a construct this printer does not know (SCSS and Less
//! interpolation, mixin definitions and guards) is `None`: the caller keeps it as written.

use super::value::{self, Grp, Item, split_commas};
use super::{Pr, text};
use crate::generated::kind as k;
use core_format::doc::{self, Doc};

impl Pr<'_, '_> {
    /// A comma separated selector list: one selector per line, or as many as fit when `flat`.
    pub fn selectors(&self, items: &[Item], keyframes: bool, flat: bool) -> Option<Doc> {
        let (segs, trailing) = split_commas(items);
        if trailing {
            return None;
        }
        let mut parts = Vec::new();
        for (i, seg) in segs.iter().enumerate() {
            if i > 0 {
                parts.push(doc::lit(","));
                parts.push(if flat { doc::line() } else { doc::hardline() });
            }
            parts.push(self.complex(seg, keyframes)?);
        }
        Some(doc::concat(parts))
    }

    fn complex(&self, items: &[Item], keyframes: bool) -> Option<Doc> {
        let mut parts: Vec<Doc> = Vec::new();
        let mut after_comb = true;
        let mut i = 0;
        while i < items.len() {
            let it = &items[i];
            let spaced = i > 0 && items[i - 1].end() != it.start();
            if let Some(op) = combinator(items, &mut i) {
                if !parts.is_empty() {
                    parts.push(doc::line());
                }
                parts.push(doc::text(op));
                if i < items.len() {
                    parts.push(doc::lit(" "));
                }
                after_comb = true;
                continue;
            }
            if spaced && !after_comb {
                parts.push(doc::line());
            }
            after_comb = false;
            match it {
                Item::Tok(t) => match t.kind() {
                    k::COLON => {
                        parts.push(self.pseudo(items, &mut i, keyframes)?);
                        continue;
                    }
                    k::IDENT if keyframes => parts.push(doc::text(keyframe_keyword(t.text()))),
                    k::IDENT
                    | k::HASH
                    | k::DELIM
                    | k::NUMBER
                    | k::DIMENSION
                    | k::PERCENTAGE
                    | k::VARIABLE => parts.push(doc::text(t.text())),
                    _ => return None,
                },
                // `#{...}` and `@{...}`: as written.
                Item::Grp(g) if g.open.kind() == k::INTERP_OPEN => {
                    parts.push(doc::text(value::raw_text(&g.node)));
                }
                Item::Grp(g) if g.open.kind() == k::L_BRACKET => parts.push(self.attribute(g)?),
                _ => return None,
            }
            i += 1;
        }
        Some(doc::group(doc::indent(doc::concat(parts))))
    }

    /// `:name`, `::name`, `:name(args)` starting at `items[*i]`.
    fn pseudo(&self, items: &[Item], i: &mut usize, keyframes: bool) -> Option<Doc> {
        let mut name = String::from(":");
        let mut j = *i + 1;
        if items.get(j)?.tok(k::COLON).is_some() && items[j - 1].end() == items[j].start() {
            name.push(':');
            j += 1;
        }
        let next = items.get(j)?;
        if items[j - 1].end() != next.start() {
            return None;
        }
        *i = j + 1;
        match next {
            Item::Tok(t) if t.kind() == k::IDENT => {
                if t.text().starts_with("--") {
                    name.push_str(t.text());
                } else {
                    name.push_str(&t.text().to_ascii_lowercase());
                }
                Some(doc::text(name))
            }
            Item::Func(t, g) => {
                name.push_str(&t.text().to_ascii_lowercase());
                let args = if name.contains("nth-") {
                    nth(g).map(doc::text)
                } else {
                    None
                };
                let args = match args {
                    Some(a) => a,
                    None => self.selectors(&g.items, keyframes, true)?,
                };
                Some(doc::concat(vec![
                    doc::text(name),
                    doc::group(doc::concat(vec![
                        doc::lit("("),
                        doc::indent(doc::concat(vec![doc::softline(), args])),
                        doc::softline(),
                        doc::lit(")"),
                    ])),
                ]))
            }
            _ => None,
        }
    }

    /// `[ a ~= b i ]` is `[a~="b" i]`.
    fn attribute(&self, g: &Grp) -> Option<Doc> {
        let mut s = String::from("[");
        let (mut op, mut value) = (false, false);
        for it in &g.items {
            let Item::Tok(t) = it else { return None };
            match t.kind() {
                k::DELIM => {
                    op |= t.text() == "=";
                    s.push_str(t.text());
                }
                k::STRING => {
                    s.push_str(&text::string(t.text(), self.single));
                    value = true;
                }
                k::IDENT | k::NUMBER | k::DIMENSION if op && !value => {
                    s.push_str(&text::string(&format!("\"{}\"", t.text()), self.single));
                    value = true;
                }
                k::IDENT if value => {
                    s.push(' ');
                    s.push_str(t.text());
                }
                _ => s.push_str(t.text()),
            }
        }
        s.push(']');
        Some(doc::text(s))
    }
}

/// A combinator (`>`, `+`, `~`, `>>>`) at `items[*i]`; moves past it.
fn combinator(items: &[Item], i: &mut usize) -> Option<String> {
    let mut op = String::new();
    while let Some(Item::Tok(t)) = items.get(*i) {
        let adjacent = op.is_empty() || items[*i - 1].end() == items[*i].start();
        if t.kind() != k::DELIM || !matches!(t.text(), ">" | "+" | "~") || !adjacent {
            break;
        }
        op.push_str(t.text());
        *i += 1;
    }
    (!op.is_empty()).then_some(op)
}

fn keyframe_keyword(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if lower == "from" || lower == "to" {
        lower
    } else {
        s.to_string()
    }
}

/// The argument of `:nth-child()` and its kin: `2N+1` is `2n + 1`. `None` when it holds anything
/// but an+b.
fn nth(g: &Grp) -> Option<String> {
    let mut s = String::new();
    let mut prev_end = None;
    for it in &g.items {
        let Item::Tok(t) = it else { return None };
        let text = t.text().to_ascii_lowercase();
        match t.kind() {
            k::DELIM if matches!(text.as_str(), "+" | "-") => {
                if s.is_empty() {
                    s.push_str(&text);
                } else {
                    s.push_str(&format!(" {text} "));
                }
            }
            k::NUMBER | k::DIMENSION | k::IDENT => {
                if prev_end.is_some_and(|e| e != t.text_range().start()) && !s.ends_with(' ') {
                    s.push(' ');
                }
                s.push_str(&text);
            }
            _ => return None,
        }
        prev_end = Some(t.text_range().end());
    }
    Some(s)
}
