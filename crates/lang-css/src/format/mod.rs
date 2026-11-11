//! CSS, SCSS and Less formatting: Prettier's output for rules, selectors, declarations, values and
//! at-rules. Preludes and values are printed from the component-value tree (`value.rs`) and
//! selectors from the same items (`selector.rs`); a construct the printer does not know is kept as
//! written. SCSS and Less constructs (control flow, mixins, maps, guards) are kept as written, and
//! their rules and declarations go through the common printer.

mod selector;
mod text;
mod value;

use crate::Dialect;
use crate::generated::kind as k;
use core_config::shared;
use core_format::comments;
use core_format::context::comment_doc;
use core_format::doc::{self, Doc};
use core_format::shapes::{dangling_comments, document, statement_sequence};
use core_format::{FormatCtx, FormatError, NodeFormatter};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange};
use value::{Item, St, items, items_with_edges, unhandled_comments};

include!("../schema.generated.rs");

pub struct CssFormatter {
    dialect: Dialect,
}

impl CssFormatter {
    pub fn new(dialect: Dialect) -> Self {
        CssFormatter { dialect }
    }
}

/// What the printer works with: the context, the dialect and the quote option.
pub(crate) struct Pr<'a, 'b> {
    cx: &'a FormatCtx<'b>,
    dialect: Dialect,
    single: bool,
}

/// At-rules whose parameters are CSS and are printed as values in every dialect.
/// Properties whose value keeps the line breaks of the source.
const GRID: &[&str] = &[
    "grid",
    "grid-area",
    "grid-template",
    "grid-template-areas",
    "grid-template-columns",
    "grid-template-rows",
];

/// SCSS at-rules whose parameters are values; those in the second list take arguments.
const SCSS_AT_RULES: &[&str] = &[
    "if", "else", "each", "for", "while", "return", "debug", "warn", "error", "use", "forward",
    "extend", "at-root",
];
const SCSS_CALLS: &[&str] = &["include", "mixin", "function", "use", "forward"];

const CSS_AT_RULES: &[&str] = &[
    "charset",
    "import",
    "namespace",
    "media",
    "supports",
    "container",
    "layer",
    "property",
    "font-face",
    "font-feature-values",
    "counter-style",
    "page",
    "viewport",
    "document",
    "scope",
    "starting-style",
    "custom-media",
    "custom-selector",
    "keyframes",
    "-webkit-keyframes",
    "-moz-keyframes",
    "-o-keyframes",
];

impl NodeFormatter for CssFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let pr = Pr {
            cx,
            dialect: self.dialect,
            single: cx.options.bool(SINGLE_QUOTE),
        };
        Ok(Some(match node.kind() {
            k::STYLESHEET => document(cx, node, cx.options.max_blank_lines(), None)?,
            k::RULE => pr.rule(node)?,
            k::AT_RULE => pr.at_rule(node)?,
            k::DECLARATION => pr.declaration(node),
            k::STATEMENT => pr.statement(node),
            k::EMPTY => pr.empty(node),
            _ => return Ok(None),
        }))
    }

    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        let tokens = |n: &SyntaxNode| -> Vec<SyntaxToken> {
            n.descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| {
                    !value::is_trivia(t.kind()) && !matches!(t.kind(), k::SEMICOLON | k::COMMA)
                })
                .collect()
        };
        let comments = |n: &SyntaxNode| -> Vec<String> {
            n.descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| matches!(t.kind(), k::COMMENT | k::LINE_COMMENT))
                .map(|t| t.text().trim_end().to_string())
                .collect()
        };
        if comments(before) != comments(after) {
            return Err("a comment was lost or changed".into());
        }
        for (a, b) in tokens(before).iter().zip(tokens(after)) {
            let (x, y) = (normalized(a), normalized(&b));
            let same = x == y || (a.kind() != b.kind() && x.eq_ignore_ascii_case(&y));
            if !same {
                return Err(format!("token `{}` became `{}`", a.text(), b.text()));
            }
        }
        Ok(())
    }
}

/// A token's text without what the formatter rewrites: case of keywords, hex digits and units,
/// the spelling of numbers, quotes and the space inside `url()`.
fn normalized(t: &SyntaxToken) -> String {
    let s = t.text();
    match t.kind() {
        k::NUMBER => text::number(s),
        k::PERCENTAGE => text::percentage(s),
        k::DIMENSION => text::dimension(s).to_ascii_lowercase(),
        k::STRING => {
            let s = text::string(s, false);
            s[1..s.len() - 1].to_string()
        }
        k::URL => text::url(s).to_ascii_lowercase(),
        k::IDENT | k::HASH | k::AT_KEYWORD => s.to_ascii_lowercase(),
        _ => s.to_string(),
    }
}

fn has_comments(node: &SyntaxNode) -> bool {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| matches!(t.kind(), k::COMMENT | k::LINE_COMMENT))
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

fn semicolon(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == k::SEMICOLON)
}

impl Pr<'_, '_> {
    /// `{`, the statements one per line and indented, `}`.
    fn block(&self, block: &SyntaxNode) -> Result<Doc, FormatError> {
        let cx = self.cx;
        let no_tokens = || FormatError::Shape("block without braces".into());
        let open = cx.first_token(block).ok_or_else(no_tokens)?;
        let close = cx.last_token(block).ok_or_else(no_tokens)?;
        let head = doc::concat(vec![self.lead(&open), doc::lit("{")]);
        let tail = doc::concat(vec![doc::lit("}"), cx.trailing_comments_doc(&close)]);
        let stmts: Vec<SyntaxNode> = block.children().collect();
        // A comment after `{` on its line is the first thing in the block.
        let opening = comments::trailing_comments(cx.grammar, &open);
        let closing = comments::leading_comments(cx.grammar, &close);
        let mut inner = vec![dangling_comments(&opening, false)];
        if !stmts.is_empty() {
            inner.push(doc::hardline());
            inner.push(statement_sequence(
                cx,
                &stmts,
                cx.options.max_blank_lines(),
            )?);
        }
        let blank_before =
            !stmts.is_empty() && comments::blank_lines_before(cx.grammar, &close) > 0;
        inner.push(dangling_comments(&closing, blank_before));
        Ok(doc::concat(vec![
            head,
            doc::indent(doc::concat(inner)),
            doc::hardline(),
            tail,
        ]))
    }

    /// The comments after a statement's `;` on its line, as text (they count for the width, as in
    /// Prettier).
    fn trail(&self, tok: &SyntaxToken) -> Doc {
        let cs = comments::trailing_comments(self.cx.grammar, tok);
        doc::concat(
            cs.iter()
                .flat_map(|c| [doc::lit(" "), comment_doc(c)])
                .collect(),
        )
    }

    /// The comments before a statement, each on its own line.
    fn lead(&self, tok: &SyntaxToken) -> Doc {
        let mut parts = Vec::new();
        for c in comments::leading_comments(self.cx.grammar, tok) {
            parts.push(comment_doc(&c));
            parts.push(doc::hardline());
            if c.breaks_after >= 2 {
                parts.push(doc::hardline());
            }
        }
        doc::concat(parts)
    }

    /// A prelude, property or value node printed by `print`; kept as written when it holds a
    /// comment inside or `print` gives up.
    fn node_with(&self, node: &SyntaxNode, print: impl FnOnce(&[Item]) -> Option<Doc>) -> Doc {
        let cx = self.cx;
        let body = if unhandled_comments(node, Some(node.text_range())) {
            None
        } else {
            print(&items(node))
        };
        match (body, cx.first_token(node), cx.last_token(node)) {
            (Some(body), Some(first), Some(last)) => doc::concat(vec![
                self.lead(&first),
                body,
                cx.trailing_comments_doc(&last),
            ]),
            _ => cx.verbatim_node(node),
        }
    }

    fn rule(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        let block = child(node, k::BLOCK)
            .ok_or_else(|| FormatError::Shape("rule without a block".into()))?;
        let keyframes = node
            .ancestors()
            .skip(1)
            .find(|a| a.kind() == k::AT_RULE)
            .and_then(|a| self.cx.first_token(&a))
            .is_some_and(|t| t.text().to_ascii_lowercase().ends_with("keyframes"));
        let mut parts = Vec::new();
        if let Some(prelude) = child(node, k::PRELUDE) {
            // What is not a selector this printer knows is kept as written (a single line in the
            // preferred quotes).
            let one_line = !value::raw_text(&prelude).contains('\n');
            parts.push(self.node_with(&prelude, |i| {
                let plain = || {
                    let comment = i
                        .iter()
                        .any(|t| t.tok(k::COMMENT).or(t.tok(k::LINE_COMMENT)).is_some());
                    (one_line && !comment).then(|| doc::text(self.plain(i)))
                };
                self.selectors(i, keyframes, false).or_else(plain)
            }));
            // Comments before the `{` go on their own lines.
            let lead = self
                .cx
                .first_token(&block)
                .is_some_and(|o| !comments::leading_comments(self.cx.grammar, &o).is_empty());
            parts.push(if lead { doc::hardline() } else { doc::lit(" ") });
        }
        parts.push(self.block(&block)?);
        Ok(doc::concat(parts))
    }

    fn at_rule(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        let cx = self.cx;
        let kw = cx
            .first_token(node)
            .ok_or_else(|| FormatError::Shape("at-rule without a name".into()))?;
        let prelude = child(node, k::PRELUDE);
        let pitems = prelude.as_ref().map(items).unwrap_or_default();
        let name = kw.text().to_ascii_lowercase();
        // `@var: value` (Less) and `@detached()` keep their name as written.
        let block = child(node, k::BLOCK);
        let sass = self.dialect == Dialect::Scss;
        let ctrl = sass && matches!(name.as_str(), "@if" | "@else" | "@each" | "@for" | "@while");
        let args = sass && SCSS_CALLS.contains(&&name[1..]);
        let css = CSS_AT_RULES.contains(&&name[1..])
            || sass && (args || SCSS_AT_RULES.contains(&&name[1..]));
        let variable = pitems.first().is_some_and(|i| {
            i.tok(k::COLON).is_some() && (!css || i.start() == kw.text_range().end())
        }) && (self.dialect == Dialect::Less || block.is_none());
        // Comments around the colon of a detached ruleset are kept as written.
        if variable
            && block.is_some()
            && (prelude.as_ref().is_some_and(has_comments)
                || node
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .skip_while(|t| t.kind() != k::AT_KEYWORD)
                    .any(|t| matches!(t.kind(), k::COMMENT | k::LINE_COMMENT)))
        {
            return Ok(doc::concat(vec![cx.verbatim_node(node), doc::lit(";")]));
        }
        let call = !css
            && matches!(pitems.first(), Some(Item::Grp(g)) if g.open.text_range().start() == kw.text_range().end());
        let mut parts = vec![
            self.lead(&kw),
            if variable || call {
                doc::text(kw.text())
            } else {
                doc::text(name.clone())
            },
            cx.trailing_comments_doc(&kw),
        ];
        if let Some(p) = &prelude {
            if variable && block.is_some() && pitems.len() == 1 {
                parts.push(doc::lit(":"));
            } else if variable && !has_comments(p) {
                parts.push(doc::lit(":"));
                let st = St {
                    soft_start: false,
                    ..St::value(true)
                };
                parts.push(self.declaration_value(&pitems[1..], st, ""));
            } else {
                if !call {
                    parts.push(doc::lit(" "));
                }
                parts.push(if css && name != "@charset" && !call {
                    let st = St {
                        media: matches!(name.as_str(), "@media" | "@custom-media"),
                        args: args || ctrl,
                        ctrl,
                        ..St::params()
                    };
                    self.node_with(p, |i| Some(self.list(i, st)))
                } else {
                    self.node_with(p, |i| {
                        if i.iter().any(|t| t.tok(k::LINE_COMMENT).is_some()) {
                            return None;
                        }
                        Some(doc::text(if name == "@charset" {
                            value::raw_text(p)
                        } else {
                            self.plain(i)
                        }))
                    })
                });
            }
        }
        match block {
            Some(block) => {
                // A control directive's `{` goes on its own line when its parameters break.
                if ctrl && prelude.is_some() {
                    let params = parts.split_off(parts.len() - 1);
                    // `@while (...) {`: a parenthesis holds the breaks.
                    let paren =
                        matches!(pitems.as_slice(), [Item::Grp(g)] if g.open.kind() == k::L_PAREN);
                    let end = if paren { doc::lit(" ") } else { doc::line() };
                    parts.push(doc::group(doc::concat(
                        params.into_iter().chain([end]).collect(),
                    )));
                } else {
                    parts.push(doc::lit(" "));
                }
                parts.push(self.block(&block)?);
                // A Less detached ruleset (`@name: { }`) ends with `;`.
                if variable {
                    parts.push(doc::lit(";"));
                }
                if let Some(next) = child(node, k::AT_RULE) {
                    parts.push(doc::lit(" "));
                    parts.push(cx.format(&next)?);
                }
            }
            None => {
                parts.push(doc::lit(";"));
                if let Some(semi) = semicolon(node) {
                    parts.push(self.trail(&semi));
                }
            }
        }
        Ok(doc::concat(parts))
    }

    /// `prop: value;` and the flags after the value.
    fn declaration(&self, node: &SyntaxNode) -> Doc {
        let cx = self.cx;
        let (Some(prop), Some(first)) = (child(node, k::PROPERTY), cx.first_token(node)) else {
            return cx.verbatim_node(node);
        };
        let pitems = items(&prop);
        let name = match pitems.as_slice() {
            [Item::Tok(t)] if t.kind() == k::IDENT => {
                if t.text().starts_with("--") || self.in_icss(node) {
                    t.text().to_string()
                } else {
                    t.text().to_ascii_lowercase()
                }
            }
            [Item::Tok(t)] if matches!(t.kind(), k::VARIABLE | k::AT_KEYWORD) => {
                t.text().to_string()
            }
            // `$var` in CSS, `*zoom`, `+color`: as written.
            [Item::Tok(t), ..] if t.kind() == k::DELIM && matches!(t.text(), "$" | "*" | "+") => {
                self.plain(&pitems)
            }
            // `--#{$prefix}-x`, `#{$side}-padding`: as written.
            [first, rest @ ..]
                if pitems.windows(2).all(|w| w[0].end() == w[1].start())
                    && pitems
                        .iter()
                        .any(|i| matches!(i, Item::Grp(g) if g.open.kind() == k::INTERP_OPEN))
                    && std::iter::once(first).chain(rest).all(|i| match i {
                        Item::Tok(t) => matches!(t.kind(), k::IDENT | k::VARIABLE),
                        Item::Grp(g) => g.open.kind() == k::INTERP_OPEN,
                        Item::Func(..) => false,
                    }) =>
            {
                self.plain(&pitems)
            }
            _ => {
                let text = cx.verbatim_node(node);
                return if semicolon(node).is_some() {
                    text
                } else {
                    doc::concat(vec![text, doc::lit(";")])
                };
            }
        };
        let value = child(node, k::VALUE);
        // What stands between the property and the value (`:` and comments) is printed as written.
        let mut colon = None;
        let mut vstart = TextRange::empty(0.into()).start();
        let mut inside = value.as_ref().map(|v| v.text_range());
        if let (Some(v), Some(end)) = (&value, pitems.last().map(|i| i.end())) {
            let first = v
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| !value::is_trivia(t.kind()));
            let from = first.as_ref().map_or(end, |t| t.text_range().start());
            let range = TextRange::new(end, from.max(end));
            let between = first
                .iter()
                .flat_map(|f| std::iter::successors(f.prev_token(), |t| t.prev_token()))
                .take_while(|t| t.text_range().start() >= end);
            let (mut block, mut line) = (false, false);
            for t in between {
                block |= t.kind() == k::COMMENT;
                line |= t.kind() == k::LINE_COMMENT;
            }
            if block && !line {
                let raw = node
                    .text()
                    .slice(range - node.text_range().start())
                    .to_string();
                colon = Some(raw.trim().to_string());
                vstart = from;
                inside = inside.map(|r| r.cover(range));
            }
        }
        if unhandled_comments(node, inside) {
            return cx.verbatim_node(node);
        }
        // `--x: { color: red }`: a block of declarations.
        if let Some(block) = value.as_ref().and_then(|v| child(v, k::BLOCK)) {
            let Ok(block) = self.block(&block) else {
                return cx.verbatim_node(node);
            };
            return doc::concat(vec![
                self.lead(&first),
                doc::text(name.clone()),
                colon.map_or(doc::lit(":"), doc::text),
                doc::lit(" "),
                block,
                doc::lit(";"),
                semicolon(node).map_or(Doc::Nil, |semi| self.trail(&semi)),
            ]);
        }
        // A custom property of blanks only keeps them.
        if value.is_none() && name.starts_with("--") {
            let blanks: String = node
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .skip_while(|t| t.kind() != k::COLON)
                .skip(1)
                .take_while(|t| t.kind() != k::SEMICOLON)
                .filter(|t| t.kind() == k::WHITESPACE)
                .map(|t| t.text().to_string())
                .collect();
            return doc::concat(vec![
                self.lead(&first),
                doc::text(format!("{name}:{blanks};")),
                semicolon(node).map_or(Doc::Nil, |semi| self.trail(&semi)),
            ]);
        }
        let mut vitems: Vec<Item> = value.map(|v| items_with_edges(&v)).unwrap_or_default();
        if colon.is_some() {
            vitems.retain(|i| i.start() >= vstart);
        }
        if matches!(vitems.first(), Some(Item::Grp(g)) if g.open.kind() == k::L_BRACE)
            || matches!(vitems.as_slice(), [Item::Tok(a), Item::Tok(b), ..]
                if a.text().eq_ignore_ascii_case("progid") && b.kind() == k::COLON)
        {
            return cx.verbatim_node(node);
        }
        let st = St {
            flat: name == "composes",
            grid: GRID.contains(&name.as_str()),
            dollar: name.starts_with('$'),
            ..St::value(!name.starts_with("--"))
        };
        doc::concat(vec![
            self.lead(&first),
            doc::text(name.clone()),
            colon.map_or(doc::lit(":"), doc::text),
            self.declaration_value(&vitems, st, ";"),
            semicolon(node).map_or(Doc::Nil, |semi| self.trail(&semi)),
        ])
    }

    /// Is the declaration in an `:import` or `:export` rule (CSS modules)?
    fn in_icss(&self, node: &SyntaxNode) -> bool {
        let selector = node
            .ancestors()
            .find(|a| a.kind() == k::RULE)
            .and_then(|r| child(&r, k::PRELUDE))
            .map(|p| value::raw_text(&p));
        selector.is_some_and(|s| s.starts_with(":import") || s.starts_with(":export"))
    }

    /// The value of a declaration with its flags (`!important`) and the closing text.
    fn declaration_value(&self, vitems: &[Item], st: St, end: &'static str) -> Doc {
        let mut n = vitems.len();
        let mut flags = Vec::new();
        while n >= 2 && vitems[n - 2].is_delim("!") && vitems[n - 1].tok(k::IDENT).is_some() {
            if let Item::Tok(t) = &vitems[n - 1] {
                flags.push(format!(" !{}", t.text().to_ascii_lowercase()));
            }
            n -= 2;
        }
        flags.reverse();
        let vitems = &vitems[..n];
        let value = self.list(vitems, st);
        doc::concat(vec![
            doc::lit(" "),
            value,
            doc::text(flags.concat()),
            doc::text(end),
        ])
    }

    /// A stray `;` with comments: they stay around it.
    fn empty(&self, node: &SyntaxNode) -> Doc {
        match self.cx.first_token(node) {
            Some(semi) => doc::concat(vec![self.lead(&semi), doc::lit(";"), self.trail(&semi)]),
            None => Doc::Nil,
        }
    }

    /// A statement that is neither a rule nor a declaration (a Less mixin call): as written, with
    /// the quotes of its strings and a `;`.
    fn statement(&self, node: &SyntaxNode) -> Doc {
        let cx = self.cx;
        let value = child(node, k::VALUE);
        // A `{ }` argument (a detached ruleset or an `each()` body) is kept as written.
        let braces = value.as_ref().is_some_and(|v| {
            v.descendants().any(|n| {
                n.kind() == k::GROUP && cx.first_token(&n).is_some_and(|t| t.kind() == k::L_BRACE)
            })
        });
        let Some(value) =
            value.filter(|v| !braces && !unhandled_comments(node, Some(v.text_range())))
        else {
            return cx.verbatim_node(node);
        };
        doc::concat(vec![
            self.node_with(&value, |i| {
                (!i.iter().any(|t| t.tok(k::LINE_COMMENT).is_some()))
                    .then(|| doc::text(self.plain(i)))
            }),
            doc::lit(";"),
            semicolon(node).map_or(Doc::Nil, |semi| self.trail(&semi)),
        ])
    }
}
