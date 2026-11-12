//! Prettier's printer-html: elements, tags with borrowed markers, children with the whitespace
//! model, text as a fill of words, attributes.

use super::model::{Dialect, N, T, Tree, is_ws, pre_like};
use core_format::EmbedMode;
use core_format::context::verbatim_text;
use core_format::doc::{self, Doc, GroupId};

mod embed;

/// Formats code of another language: language, mode, code, width, single quotes (see
/// `core_format::Embedder`); `None` when embedded formatting is off.
pub type Embed<'a> = &'a dyn Fn(&str, EmbedMode, &str, usize, bool) -> Option<String>;

pub struct Opts<'a> {
    pub dialect: Dialect,
    /// Vue and Svelte: the content of `<script>` and `<style>` is indented.
    pub indent_blocks: bool,
    pub embed: Option<Embed<'a>>,
    pub width: usize,
    pub tab: usize,
    pub bracket_same_line: bool,
    pub single_attribute_per_line: bool,
}

pub struct P<'a> {
    pub t: &'a Tree<'a>,
    pub o: Opts<'a>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bl {
    Empty,
    Soft,
    Line,
    Hard,
}

impl Bl {
    fn doc(self) -> Doc {
        match self {
            Bl::Empty => doc::nil(),
            Bl::Soft => doc::softline(),
            Bl::Line => doc::line(),
            Bl::Hard => doc::hardline(),
        }
    }
}

fn s(x: impl Into<String>) -> Doc {
    doc::text(x)
}

/// Removes the blank lines before the text and the blanks after it.
fn trim_preserve_indentation(v: &str) -> &str {
    let mut v = v.trim_end_matches(is_ws);
    while let Some(i) = v.find('\n') {
        if v[..i]
            .chars()
            .all(|c| matches!(c, '\t' | '\x0c' | '\r' | ' '))
        {
            v = &v[i + 1..];
        } else {
            break;
        }
    }
    v
}

fn dedent(v: &str) -> String {
    let indent = |l: &str| l.chars().take_while(|c| matches!(c, ' ' | '\t')).count();
    let min = v
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(indent)
        .min()
        .unwrap_or(0);
    v.split('\n')
        .map(|l| l.chars().skip(min.min(indent(l))).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn unescape_quotes(v: &str) -> String {
    v.replace("&apos;", "'").replace("&quot;", "\"")
}

impl P<'_> {
    fn n(&self, id: usize) -> &N<'_> {
        &self.t.n[id]
    }

    fn text_like(&self, id: usize) -> bool {
        matches!(self.n(id).t, T::Text | T::Comment)
    }

    /// Does the last descendant of `id` count as text? (An interpolation's content does.)
    fn ends_in_text(&self, id: usize) -> bool {
        let last = self.last_descendant(id);
        self.text_like(last) || matches!(self.n(last).t, T::Interp | T::Mustache)
    }

    fn last_descendant(&self, id: usize) -> usize {
        match self.n(id).kids.last() {
            Some(&c) => self.last_descendant(c),
            None => id,
        }
    }

    fn explicit_close(&self, id: usize) -> bool {
        self.n(id).close_start.is_some()
    }

    fn has_ignore(&self, id: usize) -> bool {
        self.n(id).prev.is_some_and(|p| {
            self.n(p).t == T::Comment
                && matches!(self.n(p).value.trim(), "prettier-ignore" | "sudsy-ignore")
        })
    }

    fn preserve(&self, id: usize) -> bool {
        pre_like(self.t, id) && self.n(id).kids.iter().any(|&c| self.n(c).t != T::Text)
    }

    // The five ways a tag borrows a marker of its neighbor.
    fn borrow_prev_close_end(&self, id: usize) -> bool {
        let n = self.n(id);
        n.prev
            .is_some_and(|p| self.n(p).t != T::Doctype && !self.text_like(p))
            && n.lead_ss
            && !n.lead_sp
    }

    fn borrow_last_child_close_end(&self, id: usize) -> bool {
        let Some(&l) = self.n(id).kids.last() else {
            return false;
        };
        let c = self.n(l);
        c.trail_ss && !c.trail_sp && !self.ends_in_text(l) && !pre_like(self.t, id)
    }

    fn borrow_parent_close_start(&self, id: usize) -> bool {
        let n = self.n(id);
        n.next.is_none() && !n.trail_sp && n.trail_ss && self.ends_in_text(id)
    }

    fn borrow_next_open_start(&self, id: usize) -> bool {
        let n = self.n(id);
        n.next.is_some_and(|x| !self.text_like(x))
            && self.text_like(id)
            && n.trail_ss
            && !n.trail_sp
    }

    fn borrow_parent_open_end(&self, id: usize) -> bool {
        let n = self.n(id);
        n.prev.is_none() && n.lead_ss && !n.lead_sp
    }

    fn should_not_print_closing(&self, id: usize) -> bool {
        let n = self.n(id);
        !n.self_closing
            && !self.explicit_close(id)
            && (self.has_ignore(id) || self.preserve(n.parent))
    }

    fn open_start_marker(&self, id: usize) -> String {
        let n = self.n(id);
        match n.t {
            T::Doctype => if n.value.eq_ignore_ascii_case("html") {
                "<!doctype"
            } else {
                "<!DOCTYPE"
            }
            .into(),
            T::Interp => "{{".into(),
            T::Mustache => "{".into(),
            _ => format!("<{}", n.name),
        }
    }

    fn open_end_marker(&self) -> &'static str {
        ">"
    }

    fn close_start_marker(&self, id: usize) -> String {
        if self.should_not_print_closing(id) {
            return String::new();
        }
        format!("</{}", self.n(id).name)
    }

    fn close_end_marker(&self, id: usize) -> &'static str {
        if self.n(id).t == T::Interp {
            "}}"
        } else if self.n(id).t == T::Mustache {
            "}"
        } else if self.should_not_print_closing(id) {
            ""
        } else if self.n(id).t == T::Element && self.n(id).self_closing {
            "/>"
        } else {
            ">"
        }
    }

    fn open_prefix(&self, id: usize) -> String {
        let n = self.n(id);
        if self.borrow_parent_open_end(id) {
            self.open_end_marker().into()
        } else if self.borrow_prev_close_end(id) {
            self.close_end_marker(n.prev.expect("prev")).into()
        } else {
            String::new()
        }
    }

    fn close_suffix(&self, id: usize) -> String {
        let n = self.n(id);
        if self.borrow_parent_close_start(id) {
            self.close_start_marker(n.parent)
        } else if self.borrow_next_open_start(id) {
            self.open_start_marker(n.next.expect("next"))
        } else {
            String::new()
        }
    }

    fn close_prefix(&self, id: usize) -> String {
        if self.borrow_last_child_close_end(id) {
            let l = *self.n(id).kids.last().expect("last child");
            self.close_end_marker(l).into()
        } else {
            String::new()
        }
    }

    fn close_start(&self, id: usize) -> String {
        if self
            .n(id)
            .kids
            .last()
            .is_some_and(|&l| self.borrow_parent_close_start(l))
        {
            String::new()
        } else {
            self.close_prefix(id) + &self.close_start_marker(id)
        }
    }

    fn close_end(&self, id: usize) -> String {
        let n = self.n(id);
        let borrowed = match n.next {
            Some(x) => self.borrow_prev_close_end(x),
            None => self.borrow_last_child_close_end(n.parent),
        };
        if borrowed {
            String::new()
        } else {
            self.close_end_marker(id).to_string() + &self.close_suffix(id)
        }
    }

    fn closing_tag(&self, id: usize) -> Doc {
        let start = if self.n(id).self_closing {
            String::new()
        } else {
            self.close_start(id)
        };
        s(start + &self.close_end(id))
    }

    fn open_start(&self, id: usize) -> String {
        if self
            .n(id)
            .prev
            .is_some_and(|p| self.borrow_next_open_start(p))
        {
            String::new()
        } else {
            self.open_prefix(id) + &self.open_start_marker(id)
        }
    }

    fn open_end(&self, id: usize) -> &'static str {
        if self
            .n(id)
            .kids
            .first()
            .is_some_and(|&f| self.borrow_parent_open_end(f))
        {
            ""
        } else {
            self.open_end_marker()
        }
    }

    // Line breaks between and inside nodes.
    fn line_of_end(&self, id: usize) -> usize {
        self.t.line(self.n(id).end)
    }

    fn has_leading_lb(&self, id: usize) -> bool {
        let n = self.n(id);
        n.lead_sp
            && match n.prev {
                Some(p) => self.line_of_end(p) < self.t.line(n.start),
                None => {
                    let p = self.n(n.parent);
                    p.t == T::Root || self.t.line(p.tag_end) < self.t.line(n.start)
                }
            }
    }

    fn has_trailing_lb(&self, id: usize) -> bool {
        let n = self.n(id);
        n.trail_sp
            && match n.next {
                Some(x) => self.t.line(self.n(x).start) > self.line_of_end(id),
                None => {
                    let p = self.n(n.parent);
                    p.t == T::Root
                        || p.close_start
                            .is_some_and(|c| self.t.line(c) > self.line_of_end(id))
                }
            }
    }

    fn surrounding_lb(&self, id: usize) -> bool {
        self.has_leading_lb(id) && self.has_trailing_lb(id)
    }

    fn prefer_hard_trailing(&self, id: usize) -> bool {
        self.surrounding_lb(id)
            || (self.n(id).t == T::Comment && self.n(id).trail_sp)
            || (self.n(id).t == T::Element && self.n(id).name == "br")
    }

    fn prefer_hard_leading(&self, id: usize) -> bool {
        self.surrounding_lb(id)
            || (self.n(id).t == T::Comment && self.n(id).lead_sp)
            || self
                .n(id)
                .prev
                .is_some_and(|p| self.prefer_hard_trailing(p))
    }

    fn force_next_empty_line(&self, id: usize) -> bool {
        self.n(id)
            .next
            .is_some_and(|x| self.line_of_end(id) + 1 < self.t.line(self.n(x).start))
    }

    fn force_break_children(&self, id: usize) -> bool {
        let n = self.n(id);
        n.t == T::Element
            && !n.kids.is_empty()
            && (matches!(n.name.as_ref(), "html" | "head" | "ul" | "ol" | "select")
                || (n.display.starts_with("table") && n.display != "table-cell"))
    }

    fn force_break_content(&self, id: usize) -> bool {
        let n = self.n(id);
        if self.force_break_children(id) {
            return true;
        }
        if n.t == T::Element
            && !n.kids.is_empty()
            && (matches!(n.name.as_ref(), "body" | "script" | "style")
                || n.kids
                    .iter()
                    .any(|&c| self.n(c).kids.iter().any(|&g| self.n(g).t != T::Text)))
        {
            return true;
        }
        match (n.kids.first(), n.kids.last()) {
            (Some(&f), Some(&l)) if f == l => {
                self.n(f).t != T::Text
                    && self.has_leading_lb(f)
                    && (!self.n(l).trail_ss || self.has_trailing_lb(l))
            }
            _ => false,
        }
    }

    fn between(&self, prev: usize, next: usize) -> Bl {
        let (p, n) = (self.n(prev), self.n(next));
        if self.text_like(prev) && self.text_like(next) {
            return if p.trail_ss {
                if p.trail_sp {
                    if self.prefer_hard_leading(next) {
                        Bl::Hard
                    } else {
                        Bl::Line
                    }
                } else {
                    Bl::Empty
                }
            } else if self.prefer_hard_leading(next) {
                Bl::Hard
            } else {
                Bl::Soft
            };
        }
        if (self.borrow_next_open_start(prev)
            && (self.has_ignore(next)
                || !n.kids.is_empty()
                || n.self_closing
                || (n.t == T::Element && !n.attrs.is_empty())))
            || (p.t == T::Element && p.self_closing && self.borrow_prev_close_end(next))
        {
            return Bl::Empty;
        }
        let deep_borrow = self.borrow_prev_close_end(next)
            && p.kids.last().is_some_and(|&l| {
                self.borrow_parent_close_start(l)
                    && self
                        .n(l)
                        .kids
                        .last()
                        .is_some_and(|&ll| self.borrow_parent_close_start(ll))
            });
        if !n.lead_ss || self.prefer_hard_leading(next) || deep_borrow {
            Bl::Hard
        } else if n.lead_sp {
            Bl::Line
        } else {
            Bl::Soft
        }
    }

    // Nodes.
    fn print_child(&self, id: usize) -> Doc {
        if self.has_ignore(id) {
            let n = self.n(id);
            let start = n.start
                + if n.prev.is_some_and(|p| self.borrow_next_open_start(p)) {
                    self.open_start_marker(id).len()
                } else {
                    0
                };
            let end = n.end
                - if n.next.is_some_and(|x| self.borrow_prev_close_end(x)) {
                    self.close_end_marker(id).len()
                } else {
                    0
                };
            return doc::concat(vec![
                s(self.open_prefix(id)),
                verbatim_text(&self.t.src[start..end.max(start)]),
                s(self.close_suffix(id)),
            ]);
        }
        if self.custom_block(id) {
            return self.custom_block_doc(id);
        }
        match self.n(id).t {
            T::Element => self.element(id),
            T::Text => self.text(id),
            T::Comment => doc::concat(vec![
                s(self.open_prefix(id)),
                verbatim_text(&self.t.src[self.n(id).start..self.n(id).end]),
                s(self.close_suffix(id)),
            ]),
            T::Doctype => {
                let n = self.n(id);
                let mut body = n.value.split_whitespace().collect::<Vec<_>>().join(" ");
                if body.len() >= 4
                    && body[..4].eq_ignore_ascii_case("html")
                    && body[4..]
                        .chars()
                        .next()
                        .is_none_or(|c| !c.is_alphanumeric() && c != '_')
                {
                    body.replace_range(..4, "html");
                }
                doc::concat(vec![
                    doc::group(s(self.open_start(id) + " " + &body)),
                    s(self.close_end(id)),
                ])
            }
            T::Interp => self.interpolation(id),
            T::Mustache => self.mustache(id),
            T::Block => self.block_doc(id),
            T::Branch => doc::nil(),
            T::Root => doc::nil(),
        }
    }

    pub fn children(&self, id: usize) -> Doc {
        let kids = &self.n(id).kids;
        if self.force_break_children(id) {
            let mut out = vec![doc::break_parent()];
            for &c in kids {
                if let Some(p) = self.n(c).prev {
                    out.push(doc::hardline());
                    if self.force_next_empty_line(p) {
                        out.push(doc::hardline());
                    }
                }
                out.push(self.print_child(c));
            }
            return doc::concat(out);
        }
        let ids: Vec<GroupId> = kids.iter().map(|_| doc::new_group_id()).collect();
        let mut out = Vec::new();
        for (i, &c) in kids.iter().enumerate() {
            let n = self.n(c);
            if self.text_like(c) {
                if let Some(p) = n.prev
                    && self.text_like(p)
                {
                    let bl = self.between(p, c);
                    if bl != Bl::Empty {
                        if self.force_next_empty_line(p) {
                            out.push(doc::concat(vec![
                                doc::hardline(),
                                doc::hardline(),
                                self.print_child(c),
                            ]));
                        } else {
                            out.push(doc::concat(vec![bl.doc(), self.print_child(c)]));
                        }
                        continue;
                    }
                }
                out.push(self.print_child(c));
                continue;
            }
            let (mut prev_parts, mut leading, mut trailing, mut next_parts) =
                (vec![], vec![], vec![], vec![]);
            if let Some(p) = n.prev {
                let bl = self.between(p, c);
                if bl != Bl::Empty {
                    if self.force_next_empty_line(p) {
                        prev_parts.extend([doc::hardline(), doc::hardline()]);
                    } else if bl == Bl::Hard {
                        prev_parts.push(doc::hardline());
                    } else if self.text_like(p) {
                        leading.push(bl.doc());
                    } else {
                        leading.push(doc::if_break_in(ids[i - 1], doc::nil(), doc::softline()));
                    }
                }
            }
            if let Some(x) = n.next {
                let bl = self.between(c, x);
                if bl != Bl::Empty {
                    if self.force_next_empty_line(c) {
                        if self.text_like(x) {
                            next_parts.extend([doc::hardline(), doc::hardline()]);
                        }
                    } else if bl == Bl::Hard {
                        if self.text_like(x) {
                            next_parts.push(doc::hardline());
                        }
                    } else {
                        trailing.push(bl.doc());
                    }
                }
            }
            let mut inner = vec![self.print_child(c)];
            inner.extend(trailing);
            leading.push(doc::group_with_id(ids[i], doc::concat(inner)));
            prev_parts.push(doc::group(doc::concat(leading)));
            prev_parts.extend(next_parts);
            out.push(doc::concat(prev_parts));
        }
        doc::concat(out)
    }

    fn text(&self, id: usize) -> Doc {
        let n = self.n(id);
        let p = self.n(n.parent);
        let (prefix, suffix) = (self.open_prefix(id), self.close_suffix(id));
        if let Some(code) = self.embedded_content(id) {
            return code;
        }
        if p.ws_sens {
            if p.indent_sens {
                return doc::concat(vec![s(prefix), verbatim_text(&n.value), s(suffix)]);
            }
            let body = dedent(trim_preserve_indentation(&n.value));
            let lines: Vec<Doc> = body.split('\n').map(s).collect();
            return doc::concat(vec![
                s(prefix),
                doc::join(&doc::hardline(), lines),
                s(suffix),
            ]);
        }
        let words: Vec<&str> = n.value.split(is_ws).filter(|w| !w.is_empty()).collect();
        let last = words.len().saturating_sub(1);
        let mut parts = Vec::new();
        for (i, w) in words.iter().enumerate() {
            if i > 0 {
                parts.push(doc::line());
            }
            let mut word = String::new();
            if i == 0 {
                word.push_str(&prefix);
            }
            word.push_str(w);
            if i == last {
                word.push_str(&suffix);
            }
            parts.push(s(word));
        }
        doc::fill(parts)
    }

    fn attribute(&self, id: usize, name: &str, value: &Option<&str>, braced: bool) -> Doc {
        let Some(v) = value else {
            return s(name);
        };
        if braced {
            return self
                .braced_attribute(id, name, v)
                .unwrap_or_else(|| doc::concat(vec![s(format!("{name}=")), verbatim_text(v)]));
        }
        if let Some(d) = self.embedded_attribute(id, name, v) {
            return d;
        }
        let v = if name.eq_ignore_ascii_case("class") && !v.contains("{{") && !v.trim().is_empty() {
            v.split(is_ws)
                .filter(|w| !w.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            unescape_quotes(v)
        };
        let single = v.matches('\'').count() < v.matches('"').count();
        let (q, v) = if single {
            ('\'', v.replace('\'', "&apos;"))
        } else {
            ('"', v.replace('"', "&quot;"))
        };
        doc::concat(vec![
            s(format!("{name}={q}")),
            verbatim_text(&v),
            s(q.to_string()),
        ])
    }

    fn attributes(&self, id: usize) -> Doc {
        let n = self.n(id);
        if n.attrs.is_empty() {
            return s(if n.self_closing { " " } else { "" });
        }
        let force_not_to_break = n.name == "script"
            && n.attrs.len() == 1
            && n.attrs[0].name.eq_ignore_ascii_case("src")
            && n.kids.is_empty();
        let per_line = self.o.single_attribute_per_line && n.attrs.len() > 1;
        let sep = if per_line {
            doc::hardline()
        } else {
            doc::line()
        };
        let printed: Vec<Doc> = n
            .attrs
            .iter()
            .map(|a| self.attribute(id, &a.name, &a.value, a.braced))
            .collect();
        let mut parts = vec![doc::indent(doc::concat(vec![
            if force_not_to_break {
                s(" ")
            } else {
                doc::line()
            },
            doc::join(&sep, printed),
        ]))];
        if n.kids
            .first()
            .is_some_and(|&f| self.borrow_parent_open_end(f))
            || (n.self_closing && self.borrow_last_child_close_end(n.parent))
            || force_not_to_break
            || self.o.bracket_same_line
        {
            parts.push(s(if n.self_closing { " " } else { "" }));
        } else {
            parts.push(if n.self_closing {
                doc::line()
            } else {
                doc::softline()
            });
        }
        doc::concat(parts)
    }

    fn opening_tag(&self, id: usize) -> Doc {
        let tail = if self.n(id).self_closing {
            ""
        } else {
            self.open_end(id)
        };
        doc::concat(vec![s(self.open_start(id)), self.attributes(id), s(tail)])
    }

    /// A root block of a Vue file other than `template`, `script` and `style`.
    fn custom_block(&self, id: usize) -> bool {
        let n = self.n(id);
        self.o.dialect == Dialect::Vue
            && n.t == T::Element
            && n.parent == 0
            && (!matches!(n.name.as_ref(), "template" | "script" | "style")
                || (n.name == "template"
                    && n.attrs
                        .iter()
                        .any(|a| a.name == "lang" && a.value != Some("html"))))
    }

    /// The content of a root `<script>` or `<style>` of a Vue file starts at the margin.
    fn unindented(&self, id: usize) -> bool {
        let n = self.n(id);
        self.o.dialect != Dialect::Html
            && !self.o.indent_blocks
            && n.parent == 0
            && matches!(n.name.as_ref(), "script" | "style")
    }

    fn depth(&self, id: usize) -> usize {
        let mut d = 0;
        let mut p = self.n(id).parent;
        while p != 0 {
            d += 1;
            p = self.n(p).parent;
        }
        d
    }

    fn element(&self, id: usize) -> Doc {
        let n = self.n(id);
        if self.preserve(id) {
            let mut start = n.tag_end;
            if n.kids
                .first()
                .is_some_and(|&f| self.borrow_parent_open_end(f))
            {
                start -= 1;
            }
            let mut end = n.close_start.unwrap_or(n.end);
            if n.kids
                .last()
                .is_some_and(|&l| self.borrow_parent_close_start(l))
            {
                end += self.close_start_marker(id).len();
            } else if self.borrow_last_child_close_end(id) {
                end -= self.close_end_marker(*n.kids.last().expect("last")).len();
            }
            return doc::concat(vec![
                doc::group(self.opening_tag(id)),
                verbatim_text(&self.t.src[start..end.max(start)]),
                self.closing_tag(id),
            ]);
        }
        let attrs = doc::new_group_id();
        let elem = |inner: Doc| {
            doc::group(doc::concat(vec![
                doc::group_with_id(attrs, self.opening_tag(id)),
                inner,
                self.closing_tag(id),
            ]))
        };
        let (Some(&first), Some(&last)) = (n.kids.first(), n.kids.last()) else {
            return elem(if n.dangling_sp && n.dangling_ss {
                doc::line()
            } else {
                doc::nil()
            });
        };
        let (f, l) = (self.n(first), self.n(last));
        // `<span>{{ expression }}</span>`: the interpolation hugs the tags unless the attributes break.
        let hug = n.kids.len() == 1
            && f.t == T::Interp
            && f.lead_ss
            && !f.lead_sp
            && l.trail_ss
            && !l.trail_sp;
        let before = if hug {
            doc::if_break_in(attrs, doc::softline(), doc::nil())
        } else if f.lead_sp && f.lead_ss {
            doc::line()
        } else if f.t == T::Text && n.ws_sens && n.indent_sens {
            doc::if_break(s("\n"), doc::nil())
        } else {
            doc::softline()
        };
        let borrowed = match n.next {
            Some(x) => self.borrow_prev_close_end(x),
            None => self.borrow_last_child_close_end(n.parent),
        };
        let after = if borrowed {
            if l.trail_sp && l.trail_ss {
                s(" ")
            } else {
                doc::nil()
            }
        } else if hug {
            doc::if_break_in(attrs, doc::softline(), doc::nil())
        } else if l.trail_sp && l.trail_ss {
            doc::line()
        } else if (l.t == T::Text && n.ws_sens && n.indent_sens)
            || (l.t == T::Comment && ends_with_indent(&l.value, self.o.tab * self.depth(id)))
        {
            doc::nil()
        } else {
            doc::softline()
        };
        elem(doc::concat(vec![
            if self.force_break_content(id) {
                doc::break_parent()
            } else {
                doc::nil()
            },
            if self.unindented(id) {
                doc::concat(vec![before, self.children(id)])
            } else if hug {
                doc::indent_if_break(attrs, doc::concat(vec![before, self.children(id)]))
            } else {
                doc::indent(doc::concat(vec![before, self.children(id)]))
            },
            after,
        ]))
    }
}

/// `\n[\t ]{width}$`
fn ends_with_indent(v: &str, width: usize) -> bool {
    v.rfind('\n').is_some_and(|i| {
        let tail = &v[i + 1..];
        tail.len() == width && tail.chars().all(|c| c == ' ' || c == '\t')
    })
}

pub fn block(t: &Tree<'_>, o: Opts<'_>) -> Doc {
    let p = P { t, o };
    doc::group(p.children(0))
}
