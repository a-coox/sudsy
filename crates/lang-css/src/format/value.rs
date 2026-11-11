//! Component-value trees (preludes, properties, values) as flat item lists, and the printer for
//! values and at-rule parameters.

use super::{Pr, text};
use crate::Dialect;
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

    pub fn is_word(&self, words: &[&str]) -> bool {
        self.tok(k::IDENT)
            .is_some_and(|t| words.contains(&t.text()))
    }

    /// The first character of `==`, `!=`, `<`, `>`, `<=` or `>=`.
    pub fn is_comparison(&self) -> bool {
        ["=", "!", "<", ">"].iter().any(|d| self.is_delim(d))
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
pub(crate) fn unhandled_comments(node: &SyntaxNode, inside: Option<TextRange>) -> bool {
    let toks: Vec<SyntaxToken> = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .collect();
    let first = toks.iter().position(|t| !is_trivia(t.kind()));
    let last = toks.iter().rposition(|t| !is_trivia(t.kind()));
    let (Some(f), Some(l)) = (first, last) else {
        return false;
    };
    toks[f..l].iter().any(|t| {
        matches!(t.kind(), k::COMMENT | k::LINE_COMMENT)
            && !inside.is_some_and(|r| r.contains_range(t.text_range()))
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
    /// At-rule parameters whose parentheses are arguments that break one per line (`@include`).
    pub args: bool,
    /// The value of a `$variable`: its SCSS maps always break.
    pub dollar: bool,
    /// The parent group is a map or the named arguments of a call (`key: value` entries); `ops`:
    /// the entry being printed has an arithmetic operator.
    pub map: bool,
    pub ops: bool,
    /// The parent group is the arguments of a function.
    pub fun: bool,
    /// The group about to print holds the arguments of a function.
    pub call: bool,
    /// The parameters of an SCSS control directive: a comparison operator stays with the word
    /// before it.
    pub ctrl: bool,
    /// The group about to print is the configuration of `@use ... with (...)`.
    pub config: bool,
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
            args: false,
            dollar: false,
            map: false,
            ops: false,
            fun: false,
            call: false,
            ctrl: false,
            config: false,
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

/// In an SCSS control directive: a space that never breaks, before a comparison or `and`, `or`,
/// `not` and after `in`, `from`, `through`, `end`.
fn glued(prev: &Item, next: &Item) -> bool {
    next.is_comparison()
        || next.is_delim("*")
        || next.is_delim("+")
        || next.is_delim("-")
        || next.is_word(&["and", "or", "not"])
        || prev.is_word(&["in", "from", "through", "end"])
}

/// Does a blank line follow the comma, which stands on the line of the item before it?
fn blank_after(tok: &SyntaxToken) -> bool {
    let mut before = tok.prev_token();
    while let Some(t) = before.clone().filter(|t| t.kind() == k::WHITESPACE) {
        before = t.prev_token();
    }
    if before.is_some_and(|t| t.kind() == k::NEWLINE) {
        return false;
    }
    let mut breaks = 0;
    let mut next = tok.next_token();
    while let Some(t) = next {
        match t.kind() {
            k::NEWLINE => breaks += 1,
            k::WHITESPACE => {}
            _ => break,
        }
        next = t.next_token();
    }
    breaks > 1
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
        // A top-level trailing comma has no use.
        let trailing = trailing && !(st.top && !st.params);
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
                // A blank line after a comma inside parentheses stays.
                let comma = segs[..=i].iter().map(|s| s.len() + 1).sum::<usize>() - 1;
                let blank =
                    st.group && matches!(items.get(comma), Some(Item::Tok(c)) if blank_after(c));
                parts.push(if blank && !st.flat {
                    doc::concat(vec![doc::line(), doc::hardline()])
                } else if st.flat {
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
        } else if st.ctrl {
            doc::group(broken(Doc::Nil, vec![doc::fill(parts)]))
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
        let st = St {
            ops: items
                .iter()
                .any(|i| ["+", "-", "*", "/", "%"].iter().any(|d| i.is_delim(d))),
            ..st
        };
        // A line comment, or a block comment with a line break after it, first in an entry is on
        // a line of its own.
        if let [first, rest @ ..] = items
            && !rest.is_empty()
            && (first.tok(k::LINE_COMMENT).is_some()
                || first.tok(k::COMMENT).is_some_and(|c| {
                    std::iter::successors(c.next_token(), |t| t.next_token())
                        .take_while(|t| matches!(t.kind(), k::WHITESPACE | k::NEWLINE))
                        .any(|t| t.text().contains('\n') || t.kind() == k::NEWLINE)
                }))
        {
            return doc::concat(vec![
                self.item(first, st),
                doc::hardline(),
                self.segment(rest, st),
            ]);
        }
        let mut parts: Vec<Doc> = Vec::new();
        let mut cur: Vec<Doc> = Vec::new();
        let colon = |i: &Item| st.group && i.tok(k::COLON).is_some();
        let gap = |a: usize, b: usize| items[a].end() != items[b].start();
        // `*` always has spaces around it, `/` only in `calc()` and when it has some already.
        // (`a*b`, `--x-*` and `*=` are words.)
        let interp =
            |i: Option<&Item>| matches!(i, Some(Item::Grp(g)) if g.open.kind() == k::INTERP_OPEN);
        let star = |j: usize| {
            items[j].is_delim("*")
                && !interp(items.get(j.wrapping_sub(1)))
                && !interp(items.get(j + 1))
                && !(j > 0
                    && !gap(j - 1, j)
                    && (items[j - 1].tok(k::IDENT).is_some()
                        || self.dialect == Dialect::Less
                            && items[j - 1].tok(k::AT_KEYWORD).is_some()))
                && !(j + 1 < items.len() && !gap(j, j + 1) && items[j + 1].is_delim("="))
        };
        // SCSS and Less `/`: spaced when the source spaces it before, after a function or
        // parenthesis, or between a number and a word; `$a/2` is a word and `$a/ 2` keeps its
        // space after.
        let slash = |j: usize| {
            let (Some(left), Some(right)) = (items.get(j.wrapping_sub(1)), items.get(j + 1)) else {
                return false;
            };
            let word =
                |i: &Item| matches!(i, Item::Tok(t) if matches!(t.kind(), k::IDENT | k::VARIABLE));
            let named = |i: &Item| word(i) || i.tok(k::AT_KEYWORD).is_some();
            let call = |i: &Item| {
                matches!(i, Item::Func(..))
                    || matches!(i, Item::Grp(g) if g.open.kind() == k::L_PAREN)
            };
            let number = |i: &Item| matches!(i, Item::Tok(t) if matches!(t.kind(), k::NUMBER | k::DIMENSION | k::PERCENTAGE));
            let plus = self.dialect != Dialect::Css
                && items[j].is_delim("+")
                && [left, right].iter().any(|i| {
                    call(i) || i.tok(k::VARIABLE).is_some() || i.tok(k::AT_KEYWORD).is_some()
                });
            plus || self.dialect != Dialect::Css
                && items[j].is_delim("/")
                && !interp(Some(left))
                && !interp(Some(right))
                && (named(left) || named(right) || call(left) || call(right))
                && (gap(j - 1, j)
                    || !gap(j, j + 1)
                        && (call(left)
                            || left.tok(k::AT_KEYWORD).is_some()
                            || number(left) && (named(right) || matches!(right, Item::Func(..))))
                    || gap(j, j + 1) && !word(left))
        };
        let muldiv = |j: usize| star(j) || slash(j) || st.calc && items[j].is_delim("/");
        // `with (...)`: the parenthesis stays on the line and breaks.
        let config = |j: usize| {
            self.dialect == Dialect::Scss
                && matches!(&items[j], Item::Grp(g) if g.open.kind() == k::L_PAREN)
                && j > 0
                && items[j - 1]
                    .tok(k::IDENT)
                    .is_some_and(|t| t.text() == "with")
        };
        let has_config = (0..items.len()).any(config);
        // A sign at the start or after an operator belongs to the number or parenthesis after it.
        let sign = |j: usize| {
            (items[j].is_delim("+") || items[j].is_delim("-"))
                && (j == 0
                    || ["+", "-", "*", "/", "%"]
                        .iter()
                        .any(|d| items[j - 1].is_delim(d)))
                && matches!(items.get(j + 1), Some(Item::Grp(g)) if g.open.kind() == k::L_PAREN)
        };
        let grid = st.grid && !st.group;
        let mut broke = false;
        let mut colon_at: Option<(usize, doc::GroupId)> = None;
        for (i, it) in items.iter().enumerate() {
            if i > 0 {
                let prev = &items[i - 1];
                // A word right after a parenthesis (`url(a)print`) is a new word.
                let after_paren = matches!(prev, Item::Func(..))
                    || prev.tok(k::URL).is_some()
                    || matches!(prev, Item::Grp(g) if g.open.kind() == k::L_PAREN);
                let word = matches!(it, Item::Tok(t) if matches!(t.kind(),
                    k::IDENT | k::STRING | k::NUMBER | k::DIMENSION | k::PERCENTAGE | k::HASH));
                // Less lookups: `@map[key]`.
                let lookup = self.dialect == Dialect::Less
                    && matches!(it, Item::Grp(g) if g.open.kind() == k::L_BRACKET);
                let after_comment = prev.tok(k::LINE_COMMENT).is_some();
                let space = if after_comment {
                    true
                } else if sign(i - 1)
                    || colon(it)
                    || colon(prev)
                    || lookup
                    || it.tok(k::SEMICOLON).is_some()
                {
                    false
                } else {
                    st.ctrl && it.is_comparison() && matches!(prev, Item::Grp(_))
                        || muldiv(i)
                        || muldiv(i - 1)
                        || prev.end() != it.start()
                        || after_paren && word
                        || prev.tok(k::STRING).is_some()
                            && (it.tok(k::STRING).is_some() || matches!(it, Item::Func(..)))
                };
                if space {
                    parts.push(doc::concat(std::mem::take(&mut cur)));
                    parts.push(if after_comment {
                        doc::hardline()
                    } else if grid {
                        let range = TextRange::new(prev.end(), it.start());
                        let parent = it.token().parent();
                        if parent.is_some_and(|p| self.cx.has_line_break(&p, range)) {
                            broke = true;
                            doc::hardline()
                        } else {
                            doc::lit(" ")
                        }
                    } else if st.flat || st.media || st.ctrl && glued(prev, it) || has_config {
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
                // The line after a colon breaks only when what follows does not fit after it.
                _ if colon(it) && (st.flat || st.media) => doc::lit(": "),
                _ if colon(it) => {
                    let id = doc::new_group_id();
                    colon_at.get_or_insert((cur.len(), id));
                    doc::group_with_id(id, doc::concat(vec![doc::lit(":"), doc::line()]))
                }
                Item::Tok(t) if lower => doc::text(t.text().to_ascii_lowercase()),
                Item::Grp(_)
                    if items
                        .get(i.wrapping_sub(1))
                        .is_some_and(|p| matches!(p, Item::Tok(t) if t.text() == "with")) =>
                {
                    self.item(it, St { config: true, ..st })
                }
                _ => self.item(it, st),
            });
        }
        // A `key: value` entry whose value does not fit goes on the next line, indented.
        if let (true, Some((at, id))) = (parts.is_empty(), colon_at) {
            let tail = cur.split_off(at + 1);
            cur.push(doc::indent_if_break(id, doc::concat(tail)));
        }
        parts.push(doc::concat(cur));
        match parts.len() {
            1 => parts.remove(0),
            _ if st.flat || st.media => doc::concat(parts),
            _ if has_config => doc::concat(parts),
            _ if broke => doc::indent(doc::concat(vec![doc::hardline(), doc::fill(parts)])),
            // Control directives break at every space or at none.
            _ if st.ctrl => doc::group(doc::indent(doc::concat(parts))),
            _ => doc::group(doc::indent(doc::fill(parts))),
        }
    }

    fn item(&self, it: &Item, st: St) -> Doc {
        match it {
            Item::Tok(t) => self.value_token(t),
            Item::Func(name, g) => {
                let lower = name.text().to_ascii_lowercase();
                let st = St {
                    calc: st.calc || lower.ends_with("calc"),
                    flat: st.flat || lower == "url",
                    ..st
                };
                doc::concat(vec![
                    self.value_token(name),
                    self.group(g, St { call: true, ..st }),
                ])
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
                let flat =
                    st.flat || st.params && !st.args && !st.config || g.open.kind() == k::L_BRACKET;
                // A SCSS map (`(key: value, ...)`) of a variable, a function argument or another
                // map breaks one entry per line, with a trailing comma.
                let entries = self.dialect == Dialect::Scss
                    && split_commas(&g.items).0[0]
                        .get(1)
                        .is_some_and(|i| i.tok(k::COLON).is_some());
                let is_map = entries && !st.call;
                let single = matches!(g.items.as_slice(), [_]);
                let force = (is_map || st.map && !single && !st.call)
                    && (st.dollar || if st.map { !st.ops } else { st.fun })
                    || st.config;
                // A trailing comma is dropped; a breaking map gets one back.
                // Comments at the end stay after the comma that breaking adds.
                let at = g
                    .items
                    .iter()
                    .rposition(|i| i.tok(k::LINE_COMMENT).is_none())
                    .map_or(0, |p| p + 1);
                let (head, comments) = g.items.split_at(at);
                let body = match head.split_last() {
                    Some((last, rest)) if last.tok(k::COMMA).is_some() => rest,
                    _ => head,
                };
                let comments = doc::concat(
                    comments
                        .iter()
                        .flat_map(|c| {
                            [
                                doc::line_suffix(doc::concat(vec![
                                    doc::lit(" "),
                                    self.item(c, st),
                                ])),
                                doc::break_parent(),
                            ]
                        })
                        .collect(),
                );
                let inner = self.list(
                    body,
                    St {
                        flat,
                        top: false,
                        group: true,
                        map: entries,
                        fun: st.call,
                        call: false,
                        config: false,
                        ..st
                    },
                );
                let (open, close) = (doc::text(g.open.text()), doc::text(g.close.text()));
                if flat {
                    doc::concat(vec![open, inner, comments, close])
                } else {
                    let comma = if force && !st.config {
                        doc::if_break(doc::lit(","), Doc::Nil)
                    } else {
                        Doc::Nil
                    };
                    doc::group(doc::concat(vec![
                        open,
                        doc::indent(doc::concat(vec![doc::softline(), inner])),
                        comma,
                        comments,
                        doc::softline(),
                        close,
                        if force { doc::break_parent() } else { Doc::Nil },
                    ]))
                }
            }
            // Interpolation: its content is a value.
            k::INTERP_OPEN => doc::concat(vec![
                doc::text(g.open.text()),
                self.list(
                    &g.items,
                    St {
                        flat: true,
                        top: false,
                        group: true,
                        ..st
                    },
                ),
                doc::text(g.close.text()),
            ]),
            // Braces: as written, but for the blanks inside them and after a colon.
            _ => {
                let raw = raw_text(&g.node);
                if raw.contains('\n') {
                    verbatim_text(&raw)
                } else {
                    doc::text(self.braces(g))
                }
            }
        }
    }

    fn braces(&self, g: &Grp) -> String {
        let mut out = String::from(g.open.text());
        let mut prev: Option<&Item> = None;
        for it in &g.items {
            let colon = it.tok(k::COLON).is_some();
            if let Some(p) = prev
                && (p.tok(k::COLON).is_some() || p.end() != it.start() && !colon)
            {
                out.push(' ');
            }
            match it {
                Item::Grp(g) if g.open.kind() == k::L_BRACE => out.push_str(&self.braces(g)),
                _ => out.push_str(&self.plain(std::slice::from_ref(it))),
            }
            prev = Some(it);
        }
        out.push_str(g.close.text());
        out
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
            k::LINE_COMMENT => return doc::concat(vec![doc::text(s), doc::break_parent()]),
            _ => s.to_string(),
        })
    }
}
