//! GraphQL formatting, laid out like Prettier: two-space indented blocks, one definition, field
//! or value per line, argument and variable lists on one line when they fit and one item per line
//! (no commas) when they do not, block strings re-indented, commas dropped.

use crate::generated::kind as k;
use core_config::shared;
use core_format::shapes::{BlockStyle, block, block_between, dangling_comments, document};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxNodeExt, SyntaxToken};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct GraphqlFormatter;

const BLOCK: BlockStyle = BlockStyle {
    max_blank_lines: 1,
    empty: "",
    force_blank: None,
    expand_empty: false,
    keep_same_line: false,
};

/// The value of a block string (`"""`): its lines without the common indentation and the blank
/// lines around them, as graphql-js reads it.
fn block_lines(text: &str) -> Vec<&str> {
    let raw = text
        .strip_prefix("\"\"\"")
        .and_then(|t| t.strip_suffix("\"\"\""))
        .unwrap_or(text);
    let lines: Vec<&str> = raw.lines().collect();
    let blank = |l: &&str| l.trim_matches([' ', '\t']).is_empty();
    let common = lines
        .iter()
        .skip(1)
        .filter(|l| !blank(l))
        .map(|l| l.len() - l.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let start = lines.iter().position(|l| !blank(l)).unwrap_or(0);
    let end = lines.iter().rposition(|l| !blank(l)).map_or(0, |i| i + 1);
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                *l
            } else {
                l.get(common..).unwrap_or("")
            }
        })
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

impl NodeFormatter for GraphqlFormatter {
    /// Only the indentation of block strings may change.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) => {
                if t.text().starts_with("\"\"\"") {
                    let lines = block_lines(t.text());
                    // A one-line value is written trimmed.
                    match lines.as_slice() {
                        [one] => one.trim().to_string().into(),
                        _ => lines.join("\n").into(),
                    }
                } else {
                    t.text().into()
                }
            }
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let doc = match node.kind() {
            k::DOCUMENT => document(cx, node, 1, None)?,
            k::OPERATION => operation(cx, node)?,
            k::FRAGMENT => fragment(cx, node)?,
            k::INLINE_FRAGMENT => inline_fragment(cx, node)?,
            k::SELECTION_SET | k::FIELDS_DEF | k::ENUM_VALUES | k::INPUT_FIELDS => {
                block(cx, node, &BLOCK)?
            }
            k::VAR_DEFS | k::ARGUMENTS | k::ARGS_DEF => paren_list(cx, node)?,
            k::FIELD => field(cx, node)?,
            k::ALIAS | k::ARGUMENT | k::OBJECT_FIELD | k::ROOT_OP => colon_spaced(cx, node)?,
            k::FIELD_DEF
            | k::INPUT_VALUE_DEF
            | k::VAR_DEF
            | k::ENUM_VALUE_DEF
            | k::FRAGMENT_SPREAD => with_type(cx, node)?,
            k::DEFAULT_VALUE => default_value(cx, node)?,
            k::LIST_VALUE => list_value(cx, node)?,
            k::OBJECT_VALUE => object_value(cx, node)?,
            k::DESCRIPTION | k::DIRECTIVE | k::TYPE_REF | k::FRAGMENT_NAME => concat(cx, node)?,
            k::TYPE_CONDITION => words(cx, node)?,
            k::SCHEMA => schema(cx, node)?,
            k::UNION_TYPE => union_type(cx, node)?,
            k::DIRECTIVE_DEF => directive_def(cx, node)?,
            k::SCALAR_TYPE | k::OBJECT_TYPE | k::INTERFACE_TYPE | k::ENUM_TYPE | k::INPUT_TYPE => {
                type_def(cx, node)?
            }
            _ => return Ok(None),
        };
        Ok(Some(doc))
    }
}

fn elements(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .by_kind(|kind| !cx.grammar.is_trivia(kind))
        .collect()
}

fn tokens_of(node: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxToken> {
    node.tokens().filter(|t| t.kind() == kind).collect()
}

fn nodes_of(node: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxNode> {
    node.children().filter(|n| n.kind() == kind).collect()
}

/// A separator token the formatter drops (`&`, `|`) keeps only the comments it carries.
fn dropped(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    doc::concat(vec![
        cx.leading_comments_doc(t),
        cx.trailing_comments_doc(t),
    ])
}

/// A token, or a block string laid out line by line at the current indentation.
fn token_doc(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    if t.kind() != k::STRING || !t.text().starts_with("\"\"\"") {
        return cx.token(t);
    }
    let lines: Vec<String> = block_lines(t.text())
        .iter()
        .map(|l| l.to_string())
        .collect();
    // Prettier: a one-line value is trimmed, a value of blank lines is empty.
    let lines = match lines.as_slice() {
        [one] => vec![one.trim().to_string()],
        _ if lines.iter().all(String::is_empty) => Vec::new(),
        _ => lines,
    };
    let mut docs = vec![doc::lit("\"\"\"")];
    docs.extend(lines.iter().map(|l| core_format::context::verbatim_text(l)));
    docs.push(doc::lit("\"\"\""));
    doc::concat(vec![
        cx.leading_comments_doc(t),
        doc::join(&doc::hardline(), docs),
        cx.trailing_comments_doc(t),
    ])
}

fn element_doc(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Result<Doc, FormatError> {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) => Ok(token_doc(cx, t)),
    }
}

fn docs_of(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> Result<Vec<Doc>, FormatError> {
    els.iter().map(|e| element_doc(cx, e)).collect()
}

/// The significant elements, one after the other with nothing between them.
fn concat(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    Ok(doc::concat(docs_of(cx, &elements(cx, node))?))
}

/// The elements one space apart.
fn words(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    Ok(doc::join(&doc::lit(" "), docs_of(cx, &elements(cx, node))?))
}

/// The elements with a space after each `:` that has something after it (`name: value`,
/// `query: Type`).
fn colon_spaced(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let els = elements(cx, node);
    let mut parts = Vec::new();
    for (i, e) in els.iter().enumerate() {
        parts.push(element_doc(cx, e)?);
        if i + 1 < els.len() && matches!(e, SyntaxElement::Token(t) if t.text() == ":") {
            parts.push(doc::lit(" "));
        }
    }
    Ok(doc::concat(parts))
}

/// `@a @b` after something: on the line when they fit, else one per line, indented; `own_line`
/// (operations and fragments) puts the first on the line too, without the indent.
fn directives(cx: &FormatCtx<'_>, node: &SyntaxNode, own_line: bool) -> Result<Doc, FormatError> {
    let ds: Result<Vec<_>, _> = nodes_of(node, k::DIRECTIVE)
        .iter()
        .map(|d| cx.format(d))
        .collect();
    let ds = ds?;
    if ds.is_empty() {
        return Ok(doc::nil());
    }
    let joined = doc::join(&doc::line(), ds);
    Ok(if own_line {
        doc::group(doc::concat(vec![doc::line(), joined]))
    } else {
        doc::concat(vec![
            doc::lit(" "),
            doc::group(doc::indent(doc::concat(vec![doc::softline(), joined]))),
        ])
    })
}

fn description(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    Ok(match nodes_of(node, k::DESCRIPTION).first() {
        // Prettier: after a one-line description of an argument a `line` (flat when the list fits).
        Some(d)
            if node.kind() == k::INPUT_VALUE_DEF && !d.text().to_string().starts_with("\"\"\"") =>
        {
            doc::concat(vec![cx.format(d)?, doc::line()])
        }
        Some(d) => doc::concat(vec![cx.format(d)?, doc::hardline()]),
        None => doc::nil(),
    })
}

/// A part with a description before it, a type after a `:` and directives at the end.
fn with_type(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = vec![description(cx, node)?];
    for e in elements(cx, node) {
        match &e {
            SyntaxElement::Node(n) if matches!(n.kind(), k::DIRECTIVE | k::DESCRIPTION) => {}
            SyntaxElement::Node(n) if n.kind() == k::TYPE_REF => {
                parts.push(doc::lit(" "));
                parts.push(cx.format(n)?);
            }
            // `...Name`
            SyntaxElement::Node(n) if n.kind() == k::FRAGMENT_NAME => parts.push(cx.format(n)?),
            _ => parts.push(element_doc(cx, &e)?),
        }
    }
    parts.push(directives(cx, node, false)?);
    Ok(doc::concat(parts))
}

/// A list of nodes between parentheses: on one line, or one per line without commas.
fn paren_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let items: Vec<SyntaxNode> = node.children().collect();
    let (Some(open), Some(close)) = (cx.first_token(node), cx.last_token(node)) else {
        return Err(FormatError::Shape("list without parentheses".into()));
    };
    let mut docs = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let mut d = vec![cx.format(item)?];
        // A blank line after an item stays.
        if let Some(next) = items.get(i + 1).and_then(|n| cx.first_token(n))
            && comments::blank_lines_before(cx.grammar, &next) > 0
        {
            d.push(doc::hardline());
        }
        docs.push(doc::concat(d));
    }
    Ok(delimited(cx, &open, &close, docs, ""))
}

/// `open`, the items one per line (separated by `, ` when it all fits one line), `close`. A
/// comment before `close` goes on its own line inside, which breaks the list.
fn delimited(
    cx: &FormatCtx<'_>,
    open: &SyntaxToken,
    close: &SyntaxToken,
    items: Vec<Doc>,
    space: &'static str,
) -> Doc {
    let sep = doc::concat(vec![
        doc::if_break(doc::nil(), doc::lit(", ")),
        doc::softline(),
    ]);
    let dangling = comments::leading_comments(cx.grammar, close);
    if dangling.is_empty() {
        return doc::group(doc::concat(vec![
            cx.token(open),
            doc::lit(space),
            doc::indent(doc::concat(vec![doc::softline(), doc::join(&sep, items)])),
            doc::softline(),
            doc::if_break(doc::nil(), doc::lit(space)),
            cx.token(close),
        ]));
    }
    let mut inner = Vec::new();
    if !items.is_empty() {
        inner.push(doc::hardline());
        inner.push(doc::join(&sep, items));
    }
    inner.push(dangling_comments(&dangling, false));
    doc::group(doc::concat(vec![
        cx.token(open),
        doc::indent(doc::concat(inner)),
        doc::hardline(),
        cx.token_text(close),
        cx.trailing_comments_doc(close),
    ]))
}

fn selection_sets(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Vec<Doc>, FormatError> {
    nodes_of(node, k::SELECTION_SET)
        .iter()
        .map(|s| cx.format(s))
        .collect()
}

fn operation(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let kw = tokens_of(node, k::KEYWORD).into_iter().next();
    let name = tokens_of(node, k::NAME).into_iter().next();
    let vars = nodes_of(node, k::VAR_DEFS);
    let mut parts = vec![description(cx, node)?];
    if let Some(kw) = &kw {
        parts.push(cx.token(kw));
    }
    if let Some(name) = &name {
        parts.push(doc::lit(" "));
        parts.push(cx.token(name));
    } else if kw.is_some() && !vars.is_empty() {
        parts.push(doc::lit(" "));
    }
    for v in &vars {
        parts.push(cx.format(v)?);
    }
    parts.push(directives(cx, node, true)?);
    if kw.is_some() || name.is_some() {
        parts.push(doc::lit(" "));
    }
    parts.extend(selection_sets(cx, node)?);
    Ok(doc::concat(parts))
}

fn fragment(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = vec![description(cx, node)?];
    for t in tokens_of(node, k::KEYWORD) {
        parts.push(cx.token(&t));
    }
    for kind in [k::FRAGMENT_NAME, k::VAR_DEFS, k::TYPE_CONDITION] {
        for n in nodes_of(node, kind) {
            if kind != k::VAR_DEFS {
                parts.push(doc::lit(" "));
            }
            parts.push(cx.format(&n)?);
        }
    }
    parts.push(directives(cx, node, true)?);
    parts.push(doc::lit(" "));
    parts.extend(selection_sets(cx, node)?);
    Ok(doc::concat(parts))
}

fn inline_fragment(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    for t in tokens_of(node, k::PUNCT) {
        parts.push(cx.token(&t));
    }
    for n in nodes_of(node, k::TYPE_CONDITION) {
        parts.push(doc::lit(" "));
        parts.push(cx.format(&n)?);
    }
    parts.push(directives(cx, node, false)?);
    parts.push(doc::lit(" "));
    parts.extend(selection_sets(cx, node)?);
    Ok(doc::concat(parts))
}

fn field(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    for e in elements(cx, node) {
        match &e {
            SyntaxElement::Node(n) if n.kind() == k::DIRECTIVE => {}
            SyntaxElement::Node(n) if n.kind() == k::SELECTION_SET => {
                parts.push(directives(cx, node, false)?);
                parts.push(doc::lit(" "));
                parts.push(cx.format(n)?);
            }
            SyntaxElement::Node(n) if n.kind() == k::ALIAS => {
                parts.push(cx.format(n)?);
                parts.push(doc::lit(" "));
            }
            _ => parts.push(element_doc(cx, &e)?),
        }
    }
    if nodes_of(node, k::SELECTION_SET).is_empty() {
        parts.push(directives(cx, node, false)?);
    }
    Ok(doc::group(doc::concat(parts)))
}

fn default_value(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let els = elements(cx, node);
    let mut parts = vec![doc::lit(" ")];
    for (i, e) in els.iter().enumerate() {
        parts.push(element_doc(cx, e)?);
        if i == 0 {
            parts.push(doc::lit(" "));
        }
    }
    Ok(doc::concat(parts))
}

fn list_value(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let els = elements(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (els.first(), els.last())
    else {
        return Err(FormatError::Shape("list without brackets".into()));
    };
    let items = docs_of(cx, &els[1..els.len() - 1])?;
    Ok(delimited(cx, open, close, items, ""))
}

fn object_value(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let fields: Result<Vec<_>, _> = nodes_of(node, k::OBJECT_FIELD)
        .iter()
        .map(|f| cx.format(f))
        .collect();
    let fields = fields?;
    let (Some(open), Some(close)) = (cx.first_token(node), cx.last_token(node)) else {
        return Err(FormatError::Shape("object without braces".into()));
    };
    let space = if cx.options.bool("bracket_spacing") && !fields.is_empty() {
        " "
    } else {
        ""
    };
    Ok(delimited(cx, &open, &close, fields, space))
}

/// `schema { query: Q }`, `extend schema @a`.
fn schema(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let keywords: Vec<Doc> = tokens_of(node, k::KEYWORD)
        .iter()
        .map(|t| cx.token(t))
        .collect();
    let mut parts = vec![
        description(cx, node)?,
        doc::join(&doc::lit(" "), keywords),
        directives(cx, node, false)?,
    ];
    if let (Some(open), Some(close)) = (
        tokens_of(node, k::L_BRACE).first(),
        tokens_of(node, k::R_BRACE).first(),
    ) {
        parts.push(doc::lit(" "));
        parts.push(block_between(cx, node, open, close, &BLOCK)?);
    }
    Ok(doc::concat(parts))
}

/// Description, keywords, name, `implements A & B`, directives and the body.
fn type_def(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let (mut head, mut interfaces, mut implements, mut body, mut seps) =
        (Vec::new(), Vec::new(), None, Vec::new(), Vec::new());
    let mut lead_ends_line = false;
    for e in elements(cx, node) {
        match &e {
            SyntaxElement::Node(n) if matches!(n.kind(), k::DESCRIPTION | k::DIRECTIVE) => {}
            SyntaxElement::Node(n) => {
                // A comment on its own line before the `{` brings its own line break.
                let own_line = cx.first_token(n).is_some_and(|t| {
                    comments::leading_comments(cx.grammar, &t)
                        .first()
                        .is_some_and(|c| {
                            c.kind != comments::CommentKind::Block || c.breaks_after > 0
                        })
                });
                let space = if own_line || lead_ends_line {
                    Doc::Nil
                } else {
                    doc::lit(" ")
                };
                body.push(doc::concat(vec![space, cx.format(n)?]));
            }
            SyntaxElement::Token(t) if t.kind() == k::AMP => {
                // Comments before a `&` end on a line of their own, where the `{` then starts.
                lead_ends_line |= !comments::leading_comments(cx.grammar, t).is_empty();
                seps.push(dropped(cx, t));
            }
            SyntaxElement::Token(t) if t.text() == "implements" => implements = Some(cx.token(t)),
            SyntaxElement::Token(t) if implements.is_some() => interfaces.push(cx.token(t)),
            SyntaxElement::Token(t) => head.push(cx.token(t)),
        }
    }
    let mut parts = vec![description(cx, node)?, doc::join(&doc::lit(" "), head)];
    if let Some(implements) = implements {
        let amp = doc::concat(vec![doc::lit(" &"), doc::line()]);
        parts.extend([doc::lit(" "), implements, doc::lit(" ")]);
        parts.push(doc::group(doc::indent(doc::join(&amp, interfaces))));
    }
    parts.extend(seps);
    parts.push(directives(cx, node, false)?);
    parts.extend(body);
    Ok(doc::concat(parts))
}

/// `union A = B | C` on one line when it fits, else `union A =` and `| B` per line.
fn union_type(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let (mut head, mut members, mut seps, mut equals) = (Vec::new(), Vec::new(), Vec::new(), false);
    for e in elements(cx, node) {
        match &e {
            SyntaxElement::Token(t) if t.kind() == k::PIPE => seps.push(dropped(cx, t)),
            SyntaxElement::Token(t) if t.text() == "=" => equals = true,
            SyntaxElement::Token(t) if equals => members.push(cx.token(t)),
            SyntaxElement::Token(t) => head.push(cx.token(t)),
            SyntaxElement::Node(_) => {}
        }
    }
    // The comments in front of the definition stay outside the group, which they would break.
    let first = (!head.is_empty()).then(|| head.remove(0));
    let mut inner = vec![
        doc::lit(" "),
        doc::join(&doc::lit(" "), head),
        directives(cx, node, false)?,
    ];
    if equals {
        let bar = doc::concat(vec![doc::line(), doc::lit("| ")]);
        inner.push(doc::lit(" ="));
        inner.push(doc::if_break(doc::nil(), doc::lit(" ")));
        inner.push(doc::indent(doc::concat(vec![
            doc::if_break(bar.clone(), doc::nil()),
            doc::join(&bar, members),
        ])));
    }
    inner.extend(seps);
    Ok(doc::concat(vec![
        description(cx, node)?,
        first.unwrap_or_else(doc::nil),
        doc::group(doc::concat(inner)),
    ]))
}

/// `directive @name(args) repeatable on A | B`.
fn directive_def(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = vec![description(cx, node)?];
    let (mut locations, mut seps) = (Vec::new(), Vec::new());
    for e in elements(cx, node) {
        match &e {
            SyntaxElement::Node(n) if n.kind() == k::ARGS_DEF => parts.push(cx.format(n)?),
            SyntaxElement::Node(n)
                if n.kind() == k::DIRECTIVE && nodes_of(node, k::DIRECTIVE).first() == Some(n) =>
            {
                parts.push(directives(cx, node, false)?);
            }
            SyntaxElement::Node(_) => {}
            SyntaxElement::Token(t) if t.kind() == k::PIPE => seps.push(dropped(cx, t)),
            SyntaxElement::Token(t) if t.kind() == k::LOCATION => locations.push(cx.token(t)),
            SyntaxElement::Token(t) if matches!(t.text(), "directive" | "extend") => {
                parts.extend([cx.token(t), doc::lit(" ")]);
            }
            SyntaxElement::Token(t) if t.text() == "on" => {
                parts.extend([doc::lit(" "), cx.token(t), doc::lit(" ")]);
            }
            SyntaxElement::Token(t) if t.text() == "repeatable" => {
                parts.extend([doc::lit(" "), cx.token(t)]);
            }
            SyntaxElement::Token(t) => parts.push(cx.token(t)),
        }
    }
    parts.push(doc::join(&doc::lit(" | "), locations));
    parts.extend(seps);
    Ok(doc::concat(parts))
}
