//! Makefile formatting: statements one per line with the source's blank lines, one space around
//! assignment operators and after rule colons, one tab before each recipe command, conditional
//! bodies indented. Values, recipe commands and `define` bodies are kept as written (trailing
//! blanks aside); recipe commands are Shell code, which the Shell language formats on its own.

use crate::generated::kind as k;
use core_config::shared;
use core_format::shapes::statement_sequence;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxNodeExt, SyntaxToken, TextRange};

include!("schema.generated.rs");

pub struct MakefileFormatter;

/// The values of assignments, the recipe prefixes and the keywords without the blanks around them (the formatter trims them).
fn values(tree: &SyntaxNode) -> Vec<String> {
    tree.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| matches!(t.kind(), k::VALUE | k::PREFIX | k::KEYWORD))
        .map(|t| t.text().trim().to_string())
        .collect()
}

impl NodeFormatter for MakefileFormatter {
    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        let (a, b) = (values(before), values(after));
        match a.iter().zip(&b).find(|(x, y)| x != y) {
            None if a.len() == b.len() => Ok(()),
            None => Err("the number of values changed".into()),
            Some((x, y)) => Err(format!("value `{x}` became `{y}`")),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::DOCUMENT => document(cx, node)?,
            k::RULE => rule(cx, node)?,
            k::RECIPE_LINE => recipe_line(cx, node, false),
            k::CONDITIONAL => conditional(cx, node)?,
            k::DEFINE => define(cx, node),
            k::ASSIGN | k::DIRECTIVE | k::CALL => {
                let mut line = Line::default();
                relay(cx, node.children_with_tokens(), &mut line);
                line.finish()
            }
            _ => return Ok(None),
        }))
    }
}

fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let items: Vec<SyntaxNode> = node.children().collect();
    let mut parts = vec![statement_sequence(
        cx,
        &items,
        cx.options.max_blank_lines(),
    )?];
    // The comments at the end of the file belong to the end-of-file token.
    let eof = node
        .last_token()
        .ok_or_else(|| FormatError::Shape("Document without tokens".into()))?;
    let trailing = comments::leading_comments(cx.grammar, &eof);
    if !trailing.is_empty() {
        if !items.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(core_format::context::leading_comments_doc(&trailing));
    }
    Ok(doc::concat(parts))
}

/// One logical line: the tokens up to the first line continuation, then the indented rest.
#[derive(Default)]
struct Line {
    head: Vec<Doc>,
    tail: Vec<Doc>,
    prev: Option<SyntaxKind>,
    /// The last character of the previous token's text.
    prev_end: char,
    blank: bool,
    broken: bool,
    /// The grammar reads `A+=b` as the name `A+` and `=`: such an operator is left as written.
    glued: bool,
}

impl Line {
    fn push(&mut self, d: Doc) {
        if self.tail.is_empty() && !self.broken {
            self.head.push(d);
        } else {
            self.tail.push(d);
        }
    }

    fn finish(self) -> Doc {
        doc::concat(vec![
            doc::concat(self.head),
            doc::indent(doc::concat(self.tail)),
        ])
    }
}

/// Appends the elements to the line: blanks collapse to one space where the source has any, and
/// the style options set the spaces around colons and assignment operators.
fn relay(cx: &FormatCtx<'_>, elements: impl Iterator<Item = SyntaxElement>, line: &mut Line) {
    for e in elements {
        let t = match e {
            SyntaxElement::Node(n) => {
                match n.kind() {
                    k::RECIPE | k::CONDITIONAL | k::ELSE => {}
                    k::DEFINE => match cx.format(&n) {
                        Ok(d) => line.push(d),
                        Err(_) => line.push(cx.verbatim_node(&n)),
                    },
                    _ => relay(cx, n.children_with_tokens(), line),
                }
                continue;
            }
            SyntaxElement::Token(t) => t,
        };
        match t.kind() {
            k::WHITESPACE if t.text().starts_with('\\') => {
                line.push(doc::lit(" \\"));
                line.broken = true;
                line.blank = false;
            }
            k::WHITESPACE | k::NEWLINE => line.blank = true,
            k::TAB | k::COMMENT => {}
            kind => {
                let glued =
                    kind == k::OP && !line.blank && matches!(line.prev_end, '+' | '?' | '!');
                let space = match (line.prev, kind) {
                    (None, _) => false,
                    (_, k::COLON) => {
                        t.text().starts_with('&') || cx.options.bool(SPACE_BEFORE_COLON)
                    }
                    (_, k::OP) if glued => false,
                    (Some(k::OP), _) if line.glued => false,
                    (Some(k::COLON | k::PIPE | k::SEMI), _) | (_, k::PIPE | k::SEMI) => true,
                    (_, k::OP) | (Some(k::OP), _) => cx.options.bool(SPACE_AROUND_ASSIGNMENT),
                    _ => line.blank,
                };
                if line.broken {
                    line.tail.push(doc::hardline());
                    line.broken = false;
                } else if space {
                    line.push(doc::lit(" "));
                }
                let d = match kind {
                    k::VALUE => value(cx, &t, line.prev == Some(k::OP)),
                    k::KEYWORD => keyword(cx, &t),
                    _ => cx.token(&t),
                };
                line.push(d);
                line.prev = Some(kind);
                line.prev_end = t.text().chars().last().unwrap_or(' ');
                line.glued = glued;
                line.blank = false;
            }
        }
    }
}

/// A keyword without the blanks before it, which its token holds.
fn keyword(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    doc::concat(vec![
        cx.leading_comments_doc(t),
        doc::text(t.text().trim()),
        cx.trailing_comments_doc(t),
    ])
}

/// An assignment's value as written, without the blanks around it; the lines after a continuation
/// keep their blanks.
fn value(cx: &FormatCtx<'_>, t: &core_syntax::SyntaxToken, assignment: bool) -> Doc {
    let text = if assignment {
        t.text().trim()
    } else {
        t.text()
    };
    doc::concat(vec![
        cx.leading_comments_doc(t),
        lines(text, false),
        cx.trailing_comments_doc(t),
    ])
}

/// Text of several lines as written, with no trailing blanks (but after a `\`, which they would
/// turn into a continuation) and no carriage returns; after the first line each starts at column 0.
fn lines(text: &str, tab: bool) -> Doc {
    let mut parts = Vec::new();
    let all: Vec<&str> = text.split('\n').collect();
    for (i, raw) in all.iter().enumerate() {
        let raw = raw.trim_end_matches('\r');
        let trimmed = raw.trim_end_matches([' ', '\t']);
        let line = if trimmed.ends_with('\\') {
            raw
        } else {
            trimmed
        };
        parts.push(match (i, tab) {
            (0, false) => doc::text(line),
            (0, true) => doc::line_start(format!("\t{line}")),
            _ => doc::line_start(line),
        });
    }
    // A last line that ends in `\` continues onto the next line, which stays empty: without it the
    // next statement would be swallowed.
    if text.ends_with('\\') {
        parts.push(doc::line_start(""));
    }
    doc::concat(parts)
}

/// `targets: prerequisites | order-only ; command`, then one command per line.
fn rule(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut head = Line::default();
    relay(cx, node.children_with_tokens(), &mut head);
    let mut parts = vec![head.finish()];
    if let Some(recipe) = node.child(k::RECIPE) {
        let mut items: Vec<SyntaxNode> = recipe.children().collect();
        // After `;` comes the command of the rule line, if the line goes on.
        let mut after_semi = recipe
            .children_with_tokens()
            .skip_while(|e| e.kind() != k::SEMI);
        if after_semi.next().is_some() {
            parts.push(doc::lit(" ;"));
            let next = after_semi.find(|e| e.kind() != k::WHITESPACE);
            if next.is_some_and(|e| e.kind() == k::RECIPE_LINE) {
                parts.push(doc::lit(" "));
                parts.push(recipe_line(cx, &items.remove(0), true));
            }
        }
        if !items.is_empty() {
            parts.push(doc::hardline());
            parts.push(statement_sequence(
                cx,
                &items,
                cx.options.max_blank_lines(),
            )?);
        }
    }
    Ok(doc::concat(parts))
}

/// The command after one tab (none for the one after `;`), as written.
fn recipe_line(cx: &FormatCtx<'_>, node: &SyntaxNode, inline: bool) -> Doc {
    let Some((_, text)) = code(node) else {
        return doc::nil();
    };
    let leading = cx
        .first_token(node)
        .map_or(doc::nil(), |t| cx.leading_comments_doc(&t));
    let text = text.trim_start_matches([' ', '\t']);
    doc::concat(vec![leading, lines(text, !inline)])
}

/// The source text of a node from its first to its last token that is not trivia, and its range.
pub(crate) fn code(node: &SyntaxNode) -> Option<(TextRange, String)> {
    let tokens: Vec<SyntaxToken> = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .collect();
    let significant =
        |t: &SyntaxToken| !matches!(t.kind(), k::WHITESPACE | k::NEWLINE | k::COMMENT);
    let first = tokens.iter().position(significant)?;
    let last = tokens.iter().rposition(significant)?;
    let text = tokens[first..=last].iter().map(|t| t.text()).collect();
    let range = tokens[first].text_range().cover(tokens[last].text_range());
    Some((range, text))
}

/// `ifeq (...)`, the body indented (not inside a recipe, where it is a command), `else`, `endif`.
fn conditional(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let in_recipe = node.ancestors().skip(1).any(|a| a.kind() == k::RECIPE);
    let body = |items: &[SyntaxNode]| -> Result<Doc, FormatError> {
        if items.is_empty() {
            return Ok(doc::nil());
        }
        let seq = doc::concat(vec![
            doc::hardline(),
            statement_sequence(cx, items, cx.options.max_blank_lines())?,
        ]);
        Ok(if in_recipe { seq } else { doc::indent(seq) })
    };
    let mut parts = Vec::new();
    let mut items = Vec::new();
    for e in node.children_with_tokens() {
        match e {
            SyntaxElement::Node(n) if n.kind() == k::DIRECTIVE && parts.is_empty() => {
                let mut line = Line::default();
                relay(cx, n.children_with_tokens(), &mut line);
                parts.push(line.finish());
            }
            SyntaxElement::Node(n) if n.kind() == k::ELSE => {
                parts.push(body(&std::mem::take(&mut items))?);
                let mut line = Line::default();
                let mut inner = Vec::new();
                relay_header(cx, &n, &mut line, &mut inner);
                parts.push(doc::hardline());
                parts.push(line.finish());
                parts.push(body(&inner)?);
            }
            SyntaxElement::Node(n) => items.push(n),
            SyntaxElement::Token(t) if t.kind() == k::KEYWORD => {
                parts.push(body(&std::mem::take(&mut items))?);
                parts.push(doc::hardline());
                parts.push(keyword(cx, &t));
            }
            SyntaxElement::Token(_) => {}
        }
    }
    Ok(doc::concat(parts))
}

/// The `else` line of an `Else` node (its keyword and the condition after it); the other children
/// are its body.
fn relay_header(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    line: &mut Line,
    body: &mut Vec<SyntaxNode>,
) {
    for e in node.children_with_tokens() {
        match e {
            // `else override A = b`: only an `if...` directive is the condition, any other starts the body
            SyntaxElement::Node(n)
                if n.kind() == k::DIRECTIVE
                    && body.is_empty()
                    && n.first_token()
                        .is_some_and(|t| t.text().trim_start().starts_with("if")) =>
            {
                relay(cx, n.children_with_tokens(), line);
            }
            SyntaxElement::Node(n) => body.push(n),
            e => relay(cx, std::iter::once(e), line),
        }
    }
}

/// `define NAME`, the body as written, `endef`.
fn define(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let mut head = Line::default();
    let mut parts = Vec::new();
    let mut elements = node.children_with_tokens().peekable();
    while let Some(e) = elements.next_if(|e| e.kind() != k::RAW && !is_endef(e)) {
        relay(cx, std::iter::once(e), &mut head);
    }
    parts.push(std::mem::take(&mut head).finish());
    for e in elements {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::RAW => {
                let body = t.text().strip_suffix('\n').unwrap_or(t.text());
                // The printer starts a line only after text; after verbatim text it takes a hardline.
                for (i, l) in body.split('\n').enumerate() {
                    let line = doc::verbatim(l.trim_end_matches('\r'));
                    let line = if i == 0 {
                        line
                    } else {
                        doc::concat(vec![doc::hardline(), line])
                    };
                    parts.push(doc::outdent(usize::MAX, line));
                }
            }
            SyntaxElement::Token(t) if t.kind() == k::KEYWORD => {
                parts.push(doc::hardline());
                parts.push(keyword(cx, &t));
            }
            _ => {}
        }
    }
    doc::concat(parts)
}

fn is_endef(e: &SyntaxElement) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.kind() == k::KEYWORD && t.text().trim() == "endef")
}
