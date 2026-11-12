//! Dockerfile formatting: one instruction per line group, the keyword upper case, one space between
//! arguments where the source has blanks, line continuations ending the line and indenting what
//! follows. Shell text, options, paths and pairs are kept as written; only the line breaks of
//! multi-line text are re-indented (never inside a quote, where the blanks are content).

use crate::generated::kind as k;
use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct DockerfileFormatter;

impl NodeFormatter for DockerfileFormatter {
    /// Only case and blanks may change; heredocs without CR.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) => match t.kind() {
                k::HEREDOC => t.text().replace('\r', "").into(),
                k::KEYWORD => t.text().to_ascii_uppercase().into(),
                // The instruction name of a raw instruction is upper-cased as well.
                k::RAW => t
                    .text()
                    .split_whitespace()
                    .collect::<String>()
                    .to_ascii_uppercase()
                    .into(),
                _ => t.text().split_whitespace().collect::<String>().into(),
            },
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::DOCUMENT => document(cx, node)?,
            k::INSTRUCTION => instruction(cx, node),
            _ => return Ok(None),
        }))
    }
}

/// The instructions, then the comments after them (owned by the end-of-file token).
fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    core_format::shapes::document(cx, node, cx.options.max_blank_lines(), None)
}

/// Lines of an instruction: the part before the first line break, the indented rest, and the
/// heredoc bodies (column 0).
#[derive(Default)]
struct Lines {
    head: Vec<Doc>,
    tail: Vec<Doc>,
    heredocs: Vec<Doc>,
}

impl Lines {
    fn push(&mut self, d: Doc) {
        if self.tail.is_empty() {
            self.head.push(d);
        } else {
            self.tail.push(d);
        }
    }
}

/// All tokens of the instruction, nested instructions (`ONBUILD`, `HEALTHCHECK ... CMD`) and
/// JSON arrays included.
fn instruction(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let mut out = Lines::default();
    let mut prev = None::<SyntaxKind>;
    let mut blank = false;
    for t in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let kind = t.kind();
        if matches!(kind, k::WHITESPACE | k::NEWLINE) {
            blank = true;
            continue;
        }
        if cx.grammar.is_trivia(kind) {
            continue;
        }
        let leading = cx.leading_comments_doc(&t);
        let text = t.text();
        if kind == k::LINE_CONTINUATION {
            out.push(doc::text(format!(" {}", &text[..1])));
            out.tail.push(doc::hardline());
            // Comment lines after the continuation are trailing trivia of it.
            for c in comments::trailing_comments(cx.grammar, &t) {
                out.tail.push(core_format::context::comment_doc(&c));
                out.tail.push(doc::hardline());
            }
            prev = None;
            blank = false;
            continue;
        }
        let space = match (prev, kind) {
            (None, _) => false,
            (_, k::COMMA | k::R_BRACKET) | (Some(k::L_BRACKET), _) => false,
            (Some(k::COMMA), _) => true,
            _ => blank,
        };
        if space {
            out.push(doc::lit(" "));
        }
        out.push(leading);
        match kind {
            k::KEYWORD => out.push(doc::text(text.to_ascii_uppercase())),
            k::RAW => {
                let (word, rest) = text.split_once([' ', '\t']).unwrap_or((text, ""));
                let text = format!("{} {}", word.to_ascii_uppercase(), rest.trim_start());
                multiline(&text, &mut out);
            }
            k::HEREDOC => out.heredocs.push(heredoc(text)),
            _ if text.contains('\n') => multiline(text, &mut out),
            _ => out.push(cx.token_text(&t)),
        }
        out.push(cx.trailing_comments_doc(&t));
        prev = Some(kind);
        blank = false;
    }
    doc::concat(vec![
        doc::concat(out.head),
        doc::indent(doc::concat(out.tail)),
        doc::concat(out.heredocs),
    ])
}

/// A heredoc body and its delimiter line, as written.
fn heredoc(text: &str) -> Doc {
    let mut parts = Vec::new();
    for (i, line) in text.replace('\r', "").split('\n').enumerate() {
        if i > 0 {
            parts.push(doc::hardline());
        }
        parts.push(doc::verbatim(line));
    }
    doc::concat(parts)
}

/// Text with line continuations: the first line goes on, every further line is indented and
/// ends in ` \` (blanks before the escape character collapse), except inside a quote, where the
/// line is kept as written. A line that starts with `#` is a comment and never opens a quote.
fn multiline(text: &str, out: &mut Lines) {
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let esc = match lines[0].trim_end().chars().last() {
        Some('`') => '`',
        _ => '\\',
    };
    let mut quote = None::<char>;
    for (i, raw) in lines.iter().enumerate() {
        let verbatim = quote.is_some();
        let line = if verbatim { raw } else { raw.trim_start() };
        let comment = !verbatim && line.starts_with('#');
        if !comment {
            let mut chars = line.chars();
            while let Some(c) = chars.next() {
                match quote {
                    _ if c == esc && quote != Some('\'') => {
                        chars.next();
                    }
                    None if c == '"' || c == '\'' => quote = Some(c),
                    Some(q) if c == q => quote = None,
                    _ => {}
                }
            }
        }
        // A line that ends inside a quote keeps its blanks before the escape character.
        let body = if quote.is_some() {
            line.trim_end_matches([' ', '\t'])
        } else {
            line.trim_end()
        };
        let body = match body.strip_suffix(esc) {
            Some(before) if quote.is_none() && !comment => {
                let before = before.trim_end();
                if before.is_empty() {
                    esc.to_string()
                } else {
                    format!("{before} {esc}")
                }
            }
            _ => body.to_string(),
        };
        match (i, verbatim) {
            (0, _) => out.push(doc::text(body)),
            (_, true) => out.tail.push(doc::line_start(body)),
            _ => {
                out.tail.push(doc::hardline());
                out.tail.push(doc::text(body));
            }
        }
    }
}
