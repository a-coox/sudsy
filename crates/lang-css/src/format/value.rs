//! Component-value trees (preludes, properties, values) as flat item lists, and the printer for
//! values and at-rule parameters.

use super::{Pr, text};
use crate::generated::kind as k;
use core_format::context::verbatim_text;
use core_format::doc::{self, Doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize};

pub(crate) enum Item {
    Tok(SyntaxToken),
    /// A name directly followed by its arguments.
    Func(SyntaxToken, Grp),
    Grp(Grp),
}

pub(crate) struct Grp {
    pub node: SyntaxNode,
    pub open: SyntaxToken,
    pub close: SyntaxToken,
    pub items: Vec<Item>,
}

pub(crate) fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::WHITESPACE | k::NEWLINE | k::COMMENT | k::LINE_COMMENT
    )
}

/// The significant children of a `Prelude`, `Property`, `Value`, `Function` or `Group` node.
/// Comments between the first and the last of them are items too.
pub(crate) fn items(node: &SyntaxNode) -> Vec<Item> {
    collect(node, false)
}

/// [`items`] with the comments before the first and after the last as well, for a node whose
/// edges nobody else prints.
pub(crate) fn items_with_edges(node: &SyntaxNode) -> Vec<Item> {
    collect(node, true)
}

fn collect(node: &SyntaxNode, edges: bool) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    let elements: Vec<SyntaxElement> = node.children_with_tokens().collect();
    let significant = |e: &SyntaxElement| match e {
        SyntaxElement::Token(t) => !is_trivia(t.kind()),
        SyntaxElement::Node(_) => true,
    };
    let first = elements.iter().position(significant).unwrap_or(0);
    let last = elements.iter().rposition(significant).unwrap_or(0);
    for (i, e) in elements.into_iter().enumerate() {
        match e {
            SyntaxElement::Token(t) if !is_trivia(t.kind()) => out.push(Item::Tok(t)),
            SyntaxElement::Token(t)
                if matches!(t.kind(), k::COMMENT | k::LINE_COMMENT)
                    && (edges || first < i && i < last) =>
            {
                out.push(Item::Tok(t));
            }
            SyntaxElement::Node(n) if matches!(n.kind(), k::GROUP | k::FUNCTION) => {
                // The comments around a nested node's tokens are in the node.
                let (lead, trail) = edge_comments(&n);
                if edges || i > first {
                    out.extend(lead.into_iter().map(Item::Tok));
                }
                if n.kind() == k::GROUP {
                    out.push(Item::Grp(grp(n)));
                } else {
                    let mut inner = items(&n).into_iter();
                    if let (Some(Item::Tok(name)), Some(Item::Grp(g))) =
                        (inner.next(), inner.next())
                    {
                        out.push(Item::Func(name, g));
                    }
                }
                if edges || i < last {
                    out.extend(trail.into_iter().map(Item::Tok));
                }
            }
            _ => {}
        }
    }
    out
}

/// The comments before the first and after the last significant token of a node.
fn edge_comments(node: &SyntaxNode) -> (Vec<SyntaxToken>, Vec<SyntaxToken>) {
    let toks: Vec<SyntaxToken> = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .collect();
    let first = toks.iter().position(|t| !is_trivia(t.kind())).unwrap_or(0);
    let last = toks.iter().rposition(|t| !is_trivia(t.kind())).unwrap_or(0);
    let comments = |range: &[SyntaxToken]| {
        range
            .iter()
            .filter(|t| matches!(t.kind(), k::COMMENT | k::LINE_COMMENT))
            .cloned()
            .collect()
    };
    (comments(&toks[..first]), comments(&toks[last + 1..]))
}

fn grp(node: SyntaxNode) -> Grp {
    let mut inner = items(&node);
    let (Some(Item::Tok(close)), true) = (inner.pop(), !inner.is_empty()) else {
        unreachable!("a group has its delimiters");
    };
    let Item::Tok(open) = inner.remove(0) else {
        unreachable!("a group starts with its opener");
    };
    Grp {
        node,
        open,
        close,
        items: inner,
    }
}

impl Item {
    pub fn start(&self) -> TextSize {
        match self {
            Item::Tok(t) | Item::Func(t, _) => t.text_range().start(),
            Item::Grp(g) => g.open.text_range().start(),
        }
    }

    pub fn end(&self) -> TextSize {
        match self {
            Item::Tok(t) => t.text_range().end(),
            Item::Func(_, g) | Item::Grp(g) => g.close.text_range().end(),
        }
    }

    pub fn token(&self) -> &SyntaxToken {
        match self {
            Item::Tok(t) | Item::Func(t, _) => t,
            Item::Grp(g) => &g.open,
        }
    }

    pub fn tok(&self, kind: SyntaxKind) -> Option<&SyntaxToken> {
        match self {
            Item::Tok(t) if t.kind() == kind => Some(t),
            _ => None,
        }
    }

    pub fn is_delim(&self, text: &str) -> bool {
        self.tok(k::DELIM).is_some_and(|t| t.text() == text)
    }
}

/// The items between top-level commas, and whether a comma ends the list.
pub(crate) fn split_commas(items: &[Item]) -> (Vec<&[Item]>, bool) {
    let mut segs: Vec<&[Item]> = items.split(|i| i.tok(k::COMMA).is_some()).collect();
    let trailing = segs.len() > 1 && segs.last().is_some_and(|s| s.is_empty());
    if trailing {
        segs.pop();
    }
    (segs, trailing)
}

/// Source text from the first to the last significant token of a node.
pub(crate) fn raw_text(node: &SyntaxNode) -> String {
    let toks: Vec<SyntaxToken> = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .collect();
    let first = toks.iter().position(|t| !is_trivia(t.kind()));
    let last = toks.iter().rposition(|t| !is_trivia(t.kind()));
    match (first, last) {
        (Some(f), Some(l)) => toks[f..=l].iter().map(|t| t.text()).collect(),
        _ => String::new(),
    }
}

/// Comments between the first and the last significant token of `node` that [`items`] does not
/// carry: a line comment, or a block comment outside `inside` (the value of a declaration).
pub(crate) fn unhandled_comments(node: &SyntaxNode, inside: Option<&SyntaxNode>) -> bool {
    let toks: Vec<SyntaxToken> = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .collect();
    let first = toks.iter().position(|t| !is_trivia(t.kind()));
    let last = toks.iter().rposition(|t| !is_trivia(t.kind()));
    let range = inside.map(|n| n.text_range());
    let (Some(f), Some(l)) = (first, last) else {
        return false;
    };
    toks[f..l].iter().any(|t| match t.kind() {
        k::LINE_COMMENT => true,
        k::COMMENT => !range.is_some_and(|r| r.contains_range(t.text_range())),
        _ => false,
    })
}

/// Where in the output a list of component values stands.
#[derive(Clone, Copy)]
pub(crate) struct St {
    /// Never broken: the inside of at-rule parameters' parentheses, of `url()` and of brackets.
    pub flat: bool,
    /// At-rule parameters: the list breaks after its commas and at its spaces, but not inside
    /// parentheses.
    pub params: bool,
    /// The top level of a declaration's value.
    pub top: bool,
    /// A top-level comma list with a multi-word item goes one item per line.
    pub break_multi: bool,
    /// Inside `calc()`: `*` and `/` get spaces.
    pub calc: bool,
    /// Inside parentheses: `:` is followed by a space.
    pub group: bool,
    /// A grid property: the line breaks of the value stay.
    pub grid: bool,
    /// Media queries: only the commas break.
    pub media: bool,
    /// A top-level comma list that does not fit starts on a new line (not after a Less variable).
    pub soft_start: bool,
}

impl St {
    /// The value of a declaration; `break_multi` is false for a custom property.
    pub fn value(break_multi: bool) -> Self {
        St {
            flat: false,
            params: false,
            top: true,
            break_multi,
            calc: false,
            group: false,
            grid: false,
            media: false,
            soft_start: true,
        }
    }

    /// The parameters of an at-rule.
    pub fn params() -> Self {
        St {
            params: true,
            break_multi: false,
            ..St::value(false)
        }
    }
}

/// Is the comma separated item more than one word? (`a b`, `1px/2px`; as Prettier's value parser
/// reads them, so is `-b` after a comma, but not `a/b`.)
fn multi_word(seg: &[Item], index: usize) -> bool {
    match seg {
        [Item::Tok(t)] => {
            index > 0
                && t.kind() == k::IDENT
                && t.text().starts_with('-')
                && !t.text().starts_with("--")
        }
        [] | [_] => false,
        // Not a word: `a/b` is one, `1px/2px` and `~"x"` are not.
        _ => {
            seg.windows(2).any(|w| w[0].end() != w[1].start())
                || !seg.iter().all(|i| {
                    matches!(i, Item::Tok(t) if matches!(t.kind(),
                        k::IDENT | k::DELIM | k::HASH | k::VARIABLE | k::AT_KEYWORD))
                })
        }
    }
}

/// Does the list print one item per line, starting on a new line (as `transition` does)?
pub(crate) fn breaks_list(items: &[Item], st: St) -> bool {
    let (segs, _) = split_commas(items);
    st.top
        && st.break_multi
        && segs.len() > 1
        && segs.iter().enumerate().any(|(i, s)| multi_word(s, i))
}

impl Pr<'_, '_> {
    /// The items as written, with runs of blanks as one space and strings in the preferred quote.
    pub fn plain(&self, items: &[Item]) -> String {
        let mut out = String::new();
        let mut end = None;
        for it in items {
            if end.is_some_and(|e| e != it.start()) {
                out.push(' ');
            }
            end = Some(it.end());
            match it {
                Item::Tok(t) if t.kind() == k::STRING => {
                    out.push_str(&text::string(t.text(), self.single))
                }
                Item::Tok(t) => out.push_str(t.text()),
                Item::Func(name, g) => {
                    out.push_str(name.text());
                    out.push_str(&self.plain_group(g));
                }
                Item::Grp(g) => out.push_str(&self.plain_group(g)),
            }
        }
        out
    }

    fn plain_group(&self, g: &Grp) -> String {
        format!(
            "{}{}{}",
            g.open.text(),
            self.plain(&g.items),
            g.close.text()
        )
    }

    pub fn list(&self, items: &[Item], st: St) -> Doc {
        let (segs, trailing) = split_commas(items);
        let inner = St { top: false, ..st };
        let mut docs: Vec<Doc> = segs.iter().map(|s| self.segment(s, inner)).collect();
        if trailing {
            let last = docs.pop().unwrap_or(Doc::Nil);
            docs.push(doc::concat(vec![last, doc::lit(",")]));
        }
        if docs.len() == 1 {
            return docs.remove(0);
        }
        // The comma belongs to the item before it, as `fill` measures the items.
        let hard = breaks_list(items, st);
        let n = docs.len();
        let mut parts = Vec::with_capacity(n * 2);
        for (i, d) in docs.into_iter().enumerate() {
            if i + 1 < n {
                parts.push(doc::concat(vec![d, doc::lit(",")]));
                parts.push(if st.flat {
                    doc::lit(" ")
                } else if hard {
                    doc::hardline()
                } else {
                    doc::line()
                });
            } else {
                parts.push(d);
            }
        }
        let broken = |first: Doc, parts: Vec<Doc>| {
            doc::indent(doc::concat(std::iter::once(first).chain(parts).collect()))
        };
        if st.flat || !st.top {
            doc::concat(parts)
        } else if hard {
            broken(doc::hardline(), parts)
        } else if st.params {
            doc::group(broken(Doc::Nil, parts))
        } else {
            let start = if st.soft_start {
                doc::softline()
            } else {
                Doc::Nil
            };
            doc::group(broken(start, vec![doc::fill(parts)]))
        }
    }

    /// Items between two commas: words separated where the source has space.
    fn segment(&self, items: &[Item], st: St) -> Doc {
        let mut parts: Vec<Doc> = Vec::new();
        let mut cur: Vec<Doc> = Vec::new();
        let colon = |i: &Item| st.group && i.tok(k::COLON).is_some();
        let gap = |a: usize, b: usize| items[a].end() != items[b].start();
        // `*` always has spaces around it, `/` only in `calc()` and when it has some already.
        // (`a*b`, `--x-*` and `*=` are words.)
        let star = |j: usize| {
            items[j].is_delim("*")
                && !(j > 0 && !gap(j - 1, j) && items[j - 1].tok(k::IDENT).is_some())
                && !(j + 1 < items.len() && !gap(j, j + 1) && items[j + 1].is_delim("="))
        };
        let muldiv = |j: usize| {
            star(j)
                || st.calc
                    && items[j].is_delim("/")
                    && (j > 0 && gap(j - 1, j) || j + 1 < items.len() && gap(j, j + 1))
        };
        let grid = st.grid && !st.group;
        let mut broke = false;
        for (i, it) in items.iter().enumerate() {
            if i > 0 {
                let prev = &items[i - 1];
                // A word right after a parenthesis (`url(a)print`) is a new word.
                let after_paren = matches!(prev, Item::Func(..))
                    || prev.tok(k::URL).is_some()
                    || matches!(prev, Item::Grp(g) if g.open.kind() == k::L_PAREN);
                let word = matches!(it, Item::Tok(t) if matches!(t.kind(),
                    k::IDENT | k::STRING | k::NUMBER | k::DIMENSION | k::PERCENTAGE | k::HASH));
                let space = if colon(it) {
                    false
                } else {
                    colon(prev)
                        || muldiv(i)
                        || muldiv(i - 1)
                        || prev.end() != it.start()
                        || after_paren && word
                };
                if space {
                    parts.push(doc::concat(std::mem::take(&mut cur)));
                    parts.push(if grid {
                        let range = TextRange::new(prev.end(), it.start());
                        let parent = it.token().parent();
                        if parent.is_some_and(|p| self.cx.has_line_break(&p, range)) {
                            broke = true;
                            doc::hardline()
                        } else {
                            doc::lit(" ")
                        }
                    } else if st.flat || st.media {
                        doc::lit(" ")
                    } else {
                        doc::line()
                    });
                }
            }
            // The name of a media feature or `:--x` in a parenthesis is lowercase.
            let lower = st.params
                && st.group
                && matches!(it, Item::Tok(t) if t.kind() == k::IDENT && !t.text().starts_with("--"))
                && items.get(i + 1).is_some_and(|n| n.tok(k::COLON).is_some());
            cur.push(match it {
                Item::Tok(t) if lower => doc::text(t.text().to_ascii_lowercase()),
                _ => self.item(it, st),
            });
        }
        parts.push(doc::concat(cur));
        match parts.len() {
            1 => parts.remove(0),
            _ if st.flat || st.media => doc::concat(parts),
            _ if broke => doc::indent(doc::concat(vec![doc::hardline(), doc::fill(parts)])),
            _ => doc::group(doc::indent(doc::fill(parts))),
        }
    }

    fn item(&self, it: &Item, st: St) -> Doc {
        match it {
            Item::Tok(t) => self.value_token(t),
            Item::Func(name, g) => {
                let lower = name.text().to_ascii_lowercase();
                // `url()` holds a quoted string or text kept as written.
                if lower == "url"
                    && !matches!(g.items.as_slice(), [Item::Tok(t)] if t.kind() == k::STRING)
                {
                    return doc::text(format!("{}{}", name.text(), self.plain_group(g)));
                }
                let st = St {
                    calc: st.calc || lower.ends_with("calc"),
                    flat: st.flat || lower == "url",
                    ..st
                };
                doc::concat(vec![self.value_token(name), self.group(g, st)])
            }
            Item::Grp(g) => self.group(g, st),
        }
    }

    fn group(&self, g: &Grp, st: St) -> Doc {
        match g.open.kind() {
            k::L_PAREN | k::L_BRACKET => {
                if g.items.is_empty() {
                    return doc::text(format!("{}{}", g.open.text(), g.close.text()));
                }
                let flat = st.flat || st.params || g.open.kind() == k::L_BRACKET;
                let inner = self.list(
                    &g.items,
                    St {
                        flat,
                        top: false,
                        group: true,
                        ..st
                    },
                );
                let (open, close) = (doc::text(g.open.text()), doc::text(g.close.text()));
                if flat {
                    doc::concat(vec![open, inner, close])
                } else {
                    doc::group(doc::concat(vec![
                        open,
                        doc::indent(doc::concat(vec![doc::softline(), inner])),
                        doc::softline(),
                        close,
                    ]))
                }
            }
            // Interpolation: its content is a value.
            k::INTERP_OPEN => doc::concat(vec![
                doc::text(g.open.text()),
                self.list(
                    &g.items,
                    St {
                        top: false,
                        group: true,
                        ..st
                    },
                ),
                doc::text(g.close.text()),
            ]),
            // Braces: as written.
            _ => verbatim_text(&raw_text(&g.node)),
        }
    }

    pub fn value_token(&self, t: &SyntaxToken) -> Doc {
        let s = t.text();
        doc::text(match t.kind() {
            k::NUMBER => text::number(s),
            k::PERCENTAGE => text::percentage(s),
            k::DIMENSION => text::dimension(s),
            k::HASH => text::hash(s),
            k::STRING => text::string(s, self.single),
            k::URL => text::url(s),
            k::IDENT => text::keyword(s),
            _ => s.to_string(),
        })
    }
}
