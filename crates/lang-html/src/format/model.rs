//! The document as Prettier's printer sees it: elements, text runs, comments and doctypes with
//! the whitespace flags of `print-preprocess.js`, built from the syntax tree and the source.

use crate::generated::kind as k;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken};
use std::borrow::Cow;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum T {
    Root,
    Element,
    Text,
    Comment,
    Doctype,
    /// Vue's `{{ expression }}` (the expression is `value`).
    Interp,
}

/// The markup language: Vue and Svelte are HTML plus template syntax.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Html,
    Vue,
}

pub struct Attr<'s> {
    pub name: Cow<'s, str>,
    /// Between the quotes (or the bare value); `None` for a boolean attribute.
    pub value: Option<&'s str>,
}

pub struct N<'s> {
    pub t: T,
    /// Element name, lowercase when it is a known HTML element.
    pub name: Cow<'s, str>,
    /// Source range of the node without surrounding whitespace.
    pub start: usize,
    pub end: usize,
    /// Element: end of the start tag, and start of the end tag when there is one.
    pub tag_end: usize,
    pub close_start: Option<usize>,
    pub attrs: Vec<Attr<'s>>,
    pub kids: Vec<usize>,
    pub parent: usize,
    pub prev: Option<usize>,
    pub next: Option<usize>,
    /// Text: the source text; comment: between `<!--` and `-->`; doctype: the body.
    pub value: Cow<'s, str>,
    pub self_closing: bool,
    pub display: &'static str,
    pub lead_sp: bool,
    pub trail_sp: bool,
    pub dangling_sp: bool,
    pub lead_ss: bool,
    pub trail_ss: bool,
    pub dangling_ss: bool,
    pub ws_sens: bool,
    pub indent_sens: bool,
    pub svg: bool,
}

impl N<'_> {
    fn new(t: T, start: usize, end: usize) -> N<'static> {
        N {
            t,
            name: Cow::Borrowed(""),
            start,
            end,
            tag_end: end,
            close_start: None,
            attrs: Vec::new(),
            kids: Vec::new(),
            parent: 0,
            prev: None,
            next: None,
            value: Cow::Borrowed(""),
            self_closing: false,
            display: "inline",
            lead_sp: false,
            trail_sp: false,
            dangling_sp: false,
            lead_ss: false,
            trail_ss: false,
            dangling_ss: false,
            ws_sens: false,
            indent_sens: false,
            svg: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Css,
    Strict,
    Ignore,
}

const KNOWN: &str = "a abbr acronym address applet area article aside audio b base basefont bdi bdo bgsound big blink blockquote body br button canvas caption center cite code col colgroup command content data datalist dd del details dfn dialog dir div dl dt element em embed fieldset figcaption figure font footer form frame frameset h1 h2 h3 h4 h5 h6 head header hgroup hr html i iframe image img input ins isindex kbd keygen label legend li link listing main map mark marquee math menu menuitem meta meter multicol nav nextid nobr noembed noframes noscript object ol optgroup option output p param picture plaintext pre progress q rb rbc rp rt rtc ruby s samp script search section select shadow slot small source spacer span strike strong style sub summary sup svg table tbody td template textarea tfoot th thead time title tr track tt u ul var video wbr xmp";

/// Attributes of HTML elements (the union of html-element-attributes): their names are lowercased.
const ATTRS: &str = "accept accept-charset accesskey action align alink allow allowfullscreen alt archive async autocapitalize autocomplete autofocus autoplay axis background bgcolor border bordercolor cellpadding cellspacing char charoff charset checked cite class classid clear code codebase codetype color cols colspan compact content contenteditable controls coords crossorigin data datetime declare decoding default defer dir dirname disabled download draggable enctype enterkeyhint face fetchpriority for form formaction formenctype formmethod formnovalidate formtarget frame frameborder headers height hidden high href hreflang hspace http-equiv id imagesizes imagesrcset inert inputmode integrity is ismap itemid itemprop itemref itemscope itemtype kind label lang language link list loading longdesc loop low marginheight marginwidth max maxlength media method min minlength multiple muted name nohref nomodule nonce noresize noshade novalidate nowrap object open optimum pattern ping placeholder playsinline popover poster preload profile prompt readonly referrerpolicy rel required rev reversed rows rowspan rules sandbox scheme scope scrolling selected shape size sizes slot span spellcheck src srcdoc srclang srcset standby start step style summary tabindex target text title translate type usemap valign value valuetype version vlink vspace width wrap";

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

pub fn is_void(name: &str) -> bool {
    VOID.contains(&name)
}

/// Prettier's table of default CSS displays (html-styles plus its own corrections).
fn default_display(name: &str) -> &'static str {
    match name {
        "html" | "body" | "address" | "blockquote" | "center" | "dialog" | "div" | "figure"
        | "figcaption" | "footer" | "form" | "header" | "hr" | "legend" | "listing" | "main"
        | "p" | "plaintext" | "pre" | "search" | "xmp" | "article" | "aside" | "h1" | "h2"
        | "h3" | "h4" | "h5" | "h6" | "hgroup" | "nav" | "section" | "dir" | "dd" | "dl" | "dt"
        | "menu" | "ol" | "ul" | "fieldset" | "details" | "summary" | "optgroup" | "option"
        | "source" | "track" | "script" | "param" => "block",
        "li" => "list-item",
        "area" | "base" | "basefont" | "datalist" | "head" | "link" | "meta" | "noembed"
        | "noframes" | "rp" | "style" | "title" => "none",
        "table" => "table",
        "caption" => "table-caption",
        "colgroup" => "table-column-group",
        "col" => "table-column",
        "thead" => "table-header-group",
        "tbody" => "table-row-group",
        "tfoot" => "table-footer-group",
        "tr" => "table-row",
        "td" | "th" => "table-cell",
        "input" | "button" | "select" | "textarea" | "meter" | "progress" | "marquee" | "video"
        | "audio" => "inline-block",
        "slot" => "contents",
        "ruby" => "ruby",
        "rt" => "ruby-text",
        _ => "inline",
    }
}

fn is_pre_like(name: &str) -> bool {
    matches!(name, "pre" | "textarea" | "listing" | "plaintext" | "xmp")
}

fn is_script_like(name: &str) -> bool {
    matches!(name, "script" | "style")
}

pub fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

pub struct Tree<'s> {
    pub src: &'s str,
    pub n: Vec<N<'s>>,
    lines: Vec<usize>,
}

struct Item {
    start: usize,
    end: usize,
    kind: SyntaxKind,
    node: Option<SyntaxNode>,
}

struct Builder<'a, 's> {
    g: &'a Grammar,
    d: Tree<'s>,
    comments: Vec<(usize, usize)>,
    mode: Mode,
    dialect: Dialect,
}

/// Range of a node's tokens that are not trivia.
fn sig(g: &Grammar, node: &SyntaxNode) -> (usize, usize) {
    let range = node.text_range();
    let first = |mut t: Option<SyntaxToken>| {
        while let Some(x) = t {
            if !g.is_trivia(x.kind()) {
                return Some(x);
            }
            t = x
                .next_token()
                .filter(|n| range.contains_range(n.text_range()));
        }
        None
    };
    let last = |mut t: Option<SyntaxToken>| {
        while let Some(x) = t {
            if !g.is_trivia(x.kind()) {
                return Some(x);
            }
            t = x
                .prev_token()
                .filter(|n| range.contains_range(n.text_range()));
        }
        None
    };
    let s = first(node.first_token()).map_or(range.start(), |t| t.text_range().start());
    let e = last(node.last_token()).map_or(range.end(), |t| t.text_range().end());
    (usize::from(s), usize::from(e))
}

pub fn build<'s>(
    g: &Grammar,
    root: &SyntaxNode,
    src: &'s str,
    mode: Mode,
    dialect: Dialect,
) -> Tree<'s> {
    let mut lines = vec![0];
    lines.extend(src.match_indices('\n').map(|(i, _)| i + 1));
    let comments = root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::COMMENT)
        .map(|t| {
            (
                usize::from(t.text_range().start()),
                usize::from(t.text_range().end()),
            )
        })
        .collect();
    let mut b = Builder {
        g,
        d: Tree {
            src,
            n: vec![N::new(T::Root, 0, src.len())],
            lines,
        },
        comments,
        mode,
        dialect,
    };
    b.children(0, root, (0, src.len()));
    b.sensitivity(0);
    b.d
}

impl Tree<'_> {
    pub fn line(&self, off: usize) -> usize {
        self.lines.partition_point(|&l| l <= off)
    }
}

impl<'s> Builder<'_, 's> {
    fn push(&mut self, n: N<'s>) -> usize {
        self.d.n.push(n);
        self.d.n.len() - 1
    }

    /// The source text of a token of the tree being built.
    fn slice(&self, t: &SyntaxToken) -> &'s str {
        let r = t.text_range();
        &self.d.src[usize::from(r.start())..usize::from(r.end())]
    }

    fn element(&mut self, node: &SyntaxNode, parent: usize, prev_comment: Option<&str>) -> usize {
        let (start, end) = sig(self.g, node);
        let mut n = N::new(T::Element, start, end);
        let mut start_tag = None;
        for c in node.children() {
            match c.kind() {
                k::START_TAG => start_tag = Some(c),
                k::END_TAG => n.close_start = Some(sig(self.g, &c).0),
                _ => {}
            }
        }
        let tag = start_tag.expect("an element has a start tag");
        let known_element = !self.d.n[parent].svg
            && tag
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .any(|t| {
                    t.kind() == k::TAG_NAME
                        && KNOWN.split(' ').any(|k| k.eq_ignore_ascii_case(t.text()))
                });
        n.tag_end = sig(self.g, &tag).1;
        let mut self_closing = false;
        for t in tag.children_with_tokens() {
            match t {
                rowan::NodeOrToken::Token(t) if t.kind() == k::TAG_NAME => {
                    n.name = Cow::Borrowed(self.slice(&t));
                }
                rowan::NodeOrToken::Token(t) if t.kind() == k::SLASH_GT => self_closing = true,
                rowan::NodeOrToken::Node(a) if a.kind() == k::ATTRIBUTE => {
                    let mut attr = Attr {
                        name: Cow::Borrowed(""),
                        value: None,
                    };
                    let (a_start, a_end) = sig(self.g, &a);
                    let name_end = (a.children_with_tokens().filter_map(|e| e.into_token()))
                        .find(|t| t.kind() == k::EQUALS)
                        .map_or(a_end, |t| usize::from(t.text_range().start()));
                    for t in a.children_with_tokens().filter_map(|e| e.into_token()) {
                        if t.kind() == k::ATTR_NAME && attr.name.is_empty() {
                            // Vue's `:a.b` and `@click` are several tokens: the name is the source before `=`.
                            attr.name = Cow::Borrowed(self.d.src[a_start..name_end].trim_end());
                            if known_element
                                && self.dialect == Dialect::Html
                                && attr.name.bytes().any(|b| b.is_ascii_uppercase())
                                && ATTRS.split(' ').any(|a| a.eq_ignore_ascii_case(&attr.name))
                            {
                                attr.name = Cow::Owned(attr.name.to_ascii_lowercase());
                            }
                        } else if t.kind() == k::ATTR_VALUE {
                            let v = self.slice(&t);
                            let inner = match v.as_bytes().first() {
                                Some(b'"' | b'\'') if v.len() >= 2 => &v[1..v.len() - 1],
                                _ => v,
                            };
                            attr.value = Some(inner);
                        }
                    }
                    n.attrs.push(attr);
                }
                _ => {}
            }
        }
        let parent_svg = self.d.n[parent].svg;
        n.svg = n.name.eq_ignore_ascii_case("svg")
            || (parent_svg && self.d.n[parent].name != "foreignObject");
        if !n.svg
            && self.dialect == Dialect::Html
            && n.name.bytes().any(|b| b.is_ascii_uppercase())
            && KNOWN.split(' ').any(|k| k.eq_ignore_ascii_case(&n.name))
        {
            n.name = Cow::Owned(n.name.to_ascii_lowercase());
        }
        n.self_closing = self_closing || (!n.svg && is_void(&n.name));
        n.parent = parent;
        let id = self.push(n);
        self.display(id, prev_comment);
        let n = &self.d.n[id];
        let region = (n.tag_end, n.close_start.unwrap_or(end).max(n.tag_end));
        self.children(id, node, region);
        id
    }

    fn display(&mut self, id: usize, prev_comment: Option<&str>) {
        let n = &self.d.n[id];
        let d = if let Some(d) = prev_comment.and_then(comment_display) {
            d
        } else if n.svg && n.name != "svg" && self.d.n[n.parent].name != "foreignObject" {
            "block"
        } else if n.svg {
            "inline-block"
        } else {
            match self.mode {
                _ if self.dialect == Dialect::Vue && n.parent == 0 => "block",
                Mode::Strict => "inline",
                Mode::Ignore => "block",
                Mode::Css => default_display(&n.name),
            }
        };
        self.d.n[id].display = d;
    }

    fn children(&mut self, parent: usize, node: &SyntaxNode, region: (usize, usize)) {
        let mut items: Vec<Item> = Vec::new();
        for c in node.children() {
            match c.kind() {
                k::CHUNK | k::ELEMENT | k::DOCTYPE | k::STRAY_END_TAG | k::INTERPOLATION => {
                    let (s, e) = sig(self.g, &c);
                    if c.kind() == k::CHUNK && self.d.src[s..e].trim_matches(is_ws).is_empty() {
                        continue;
                    }
                    items.push(Item {
                        start: s,
                        end: e,
                        kind: c.kind(),
                        node: Some(c),
                    });
                }
                _ => {}
            }
        }
        let at = self.comments.partition_point(|c| c.0 < region.0);
        let mut extra = Vec::new();
        for &(s, e) in &self.comments[at..] {
            if s >= region.1 {
                break;
            }
            if !items.iter().any(|i| i.start <= s && e <= i.end) {
                extra.push(Item {
                    start: s,
                    end: e,
                    kind: k::COMMENT,
                    node: None,
                });
            }
        }
        items.extend(extra);
        items.sort_by_key(|i| i.start);
        let ws_sens = {
            let p = &self.d.n[parent];
            p.t == T::Element && (is_script_like(&p.name) || is_pre_like(&p.name))
        };
        self.d.n[parent].ws_sens = ws_sens;
        self.d.n[parent].indent_sens =
            self.d.n[parent].t == T::Element && is_pre_like(&self.d.n[parent].name);
        let mut kids: Vec<usize> = Vec::new();
        let mut i = 0;
        while i < items.len() {
            let it = &items[i];
            let id = match it.kind {
                k::ELEMENT => {
                    let inner = (i > 0 && items[i - 1].kind == k::COMMENT)
                        .then(|| &self.d.src[items[i - 1].start..items[i - 1].end])
                        .and_then(|c| c.strip_prefix("<!--")?.strip_suffix("-->"));
                    self.element(it.node.as_ref().expect("element"), parent, inner)
                }
                k::COMMENT => {
                    let raw = &self.d.src[it.start..it.end];
                    let mut n = N::new(T::Comment, it.start, it.end);
                    let raw = raw.strip_prefix("<!--").unwrap_or(raw);
                    n.value = Cow::Borrowed(raw.strip_suffix("-->").unwrap_or(raw));
                    n.parent = parent;
                    self.push(n)
                }
                k::INTERPOLATION => {
                    let mut n = N::new(T::Interp, it.start, it.end);
                    let inner = &self.d.src[it.start..it.end];
                    let inner = inner.strip_prefix("{{").unwrap_or(inner);
                    n.value = Cow::Borrowed(inner.strip_suffix("}}").unwrap_or(inner));
                    n.parent = parent;
                    self.push(n)
                }
                k::DOCTYPE => {
                    let mut n = N::new(T::Doctype, it.start, it.end);
                    let body = it
                        .node
                        .as_ref()
                        .expect("doctype")
                        .children_with_tokens()
                        .filter_map(|e| e.into_token())
                        .find(|t| t.kind() == k::DOCTYPE_BODY)
                        .map_or("", |t| self.slice(&t).trim());
                    n.value = Cow::Borrowed(body);
                    n.parent = parent;
                    self.push(n)
                }
                _ => {
                    // A run of text, entities and CDATA sections with nothing but blanks between.
                    let mut j = i;
                    while j + 1 < items.len()
                        && matches!(items[j + 1].kind, k::CHUNK | k::STRAY_END_TAG)
                    {
                        j += 1;
                    }
                    // A text may carry the blanks around it (Vue's grammar).
                    let text = &self.d.src[it.start..items[j].end];
                    let lead = text.len() - text.trim_start_matches(is_ws).len();
                    let (s, e) = (
                        it.start + lead,
                        items[j].end
                            - (text.len() - lead - text[lead..].trim_end_matches(is_ws).len()),
                    );
                    let mut n = N::new(T::Text, s, e);
                    n.value = Cow::Borrowed(&self.d.src[s..e]);
                    n.parent = parent;
                    i = j;
                    self.push(n)
                }
            };
            kids.push(id);
            i += 1;
        }
        self.d.n[parent].kids = kids;
        self.whitespace(parent, region);
    }

    /// `extractWhitespaces`: the flags for blanks between the children, and the text of a
    /// whitespace-sensitive element kept whole.
    fn whitespace(&mut self, parent: usize, region: (usize, usize)) {
        let ws_sens = self.d.n[parent].ws_sens;
        let kids = self.d.n[parent].kids.clone();
        let all_text = kids.iter().all(|&c| self.d.n[c].t == T::Text);
        if ws_sens && all_text && !kids.is_empty() {
            // The text keeps its blanks; the first line break of `pre` and `textarea` is not content.
            let mut s = region.0;
            if self.d.n[parent].indent_sens && self.d.src[s..region.1].starts_with('\n') {
                s += 1;
            }
            if s >= region.1 {
                self.d.n[parent].kids = Vec::new();
                return;
            }
            let t = kids[0];
            let n = &mut self.d.n[t];
            n.start = s;
            n.end = region.1;
            n.value = Cow::Borrowed(&self.d.src[s..region.1]);
            self.d.n[parent].kids = vec![t];
            return;
        }
        if kids.is_empty() {
            self.d.n[parent].dangling_sp = region.0 < region.1;
            return;
        }
        for (i, &c) in kids.iter().enumerate() {
            let before = if i == 0 {
                region.0
            } else {
                self.d.n[kids[i - 1]].end
            };
            let after = if i + 1 == kids.len() {
                region.1
            } else {
                self.d.n[kids[i + 1]].start
            };
            let (s, e) = (self.d.n[c].start, self.d.n[c].end);
            self.d.n[c].lead_sp = before < s && self.d.src[before..s].chars().any(is_ws);
            self.d.n[c].trail_sp = e < after && self.d.src[e..after].chars().any(is_ws);
            self.d.n[c].prev = (i > 0).then(|| kids[i - 1]);
            self.d.n[c].next = kids.get(i + 1).copied();
        }
    }

    fn sensitivity(&mut self, id: usize) {
        let kids = self.d.n[id].kids.clone();
        if kids.is_empty() {
            let n = &self.d.n[id];
            self.d.n[id].dangling_ss = !block_like(n.display)
                && n.display != "inline-block"
                && !(n.t == T::Element && is_script_like(&n.name));
            return;
        }
        for &c in &kids {
            let l = self.leading(c);
            let t = self.trailing(c);
            self.d.n[c].lead_ss = l;
            self.d.n[c].trail_ss = t;
        }
        for (i, &c) in kids.iter().enumerate() {
            let prev_t = i == 0 || self.d.n[kids[i - 1]].trail_ss;
            let next_l = i + 1 == kids.len() || self.d.n[kids[i + 1]].lead_ss;
            self.d.n[c].lead_ss = prev_t && self.d.n[c].lead_ss;
            self.d.n[c].trail_ss = next_l && self.d.n[c].trail_ss;
        }
        for c in kids {
            self.sensitivity(c);
        }
    }

    fn leading(&self, id: usize) -> bool {
        let n = &self.d.n[id];
        let p = &self.d.n[n.parent];
        let r = (|| {
            if n.t == T::Text && n.prev.is_some_and(|q| self.d.n[q].t == T::Text) {
                return true;
            }
            if block_like(n.display) {
                return false;
            }
            if p.display == "none" && p.t == T::Element {
                return false;
            }
            if p.t == T::Element && is_pre_like(&p.name) {
                return true;
            }
            let pre_like = n.t == T::Element && is_pre_like(&n.name);
            if n.prev.is_none()
                && (p.t == T::Root
                    || pre_like
                    || (p.t == T::Element && is_script_like(&p.name))
                    || block_like(p.display)
                    || p.display == "inline-block")
            {
                return false;
            }
            if let Some(q) = n.prev
                && block_like(self.d.n[q].display)
            {
                return false;
            }
            !block_like(n.display)
        })();
        if r && n.prev.is_none()
            && p.t == T::Element
            && matches!(p.name.as_ref(), "pre" | "textarea" | "listing")
        {
            return false;
        }
        r
    }

    fn trailing(&self, id: usize) -> bool {
        let n = &self.d.n[id];
        let p = &self.d.n[n.parent];
        if n.t == T::Text && n.next.is_some_and(|q| self.d.n[q].t == T::Text) {
            return true;
        }
        if block_like(n.display) {
            return false;
        }
        if p.display == "none" && p.t == T::Element {
            return false;
        }
        if p.t == T::Element && is_pre_like(&p.name) {
            return true;
        }
        let pre_like = n.t == T::Element && is_pre_like(&n.name);
        if n.next.is_none()
            && (p.t == T::Root
                || pre_like
                || (p.t == T::Element && is_script_like(&p.name))
                || block_like(p.display)
                || p.display == "inline-block")
        {
            return false;
        }
        if let Some(q) = n.next
            && block_like(self.d.n[q].display)
        {
            return false;
        }
        !block_like(n.display)
    }
}

/// `display: block` in a comment before an element.
fn comment_display(c: &str) -> Option<&'static str> {
    let v = c.trim().strip_prefix("display:")?.trim();
    [
        "block",
        "inline",
        "inline-block",
        "none",
        "list-item",
        "contents",
    ]
    .into_iter()
    .find(|d| *d == v)
}

pub fn block_like(d: &str) -> bool {
    d == "block" || d == "list-item" || d.starts_with("table")
}

pub fn pre_like(d: &Tree, id: usize) -> bool {
    let n = &d.n[id];
    n.t == T::Element && is_pre_like(&n.name)
}
