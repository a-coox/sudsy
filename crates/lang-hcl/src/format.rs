//! HCL formatting, the layout of `terraform fmt` (hashicorp/hcl `hclwrite.Format`): every source
//! line stays a line (blank lines and line breaks inside expressions are kept), and on each line
//! the indentation follows the brackets, the spaces between tokens follow fixed rules, and the `=`
//! of consecutive attributes and their trailing comments line up. The tree only classifies the
//! tokens; the layout works on the token stream, as hclwrite does.

use crate::generated::kind as k;
use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};
use rowan::WalkEvent;

include!("schema.generated.rs");

pub struct HclFormatter;

impl NodeFormatter for HclFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        if node.kind() != k::DOCUMENT {
            return Ok(None);
        }
        let mut lines = lines(node);
        indent(
            &mut lines,
            cx.options.int(shared::INDENT_SIZE).max(1) as usize,
        );
        let mut out = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            line.print(&mut out);
        }
        // Blank lines at the end stay, as the blanks after a heredoc's closing marker do.
        Ok(Some(doc::verbatim(out)))
    }
}

/// What a token is for the spacing rules (hclwrite's token types).
#[derive(Clone, Copy, PartialEq)]
enum T {
    Ident,
    Keyword,
    Dot,
    Comma,
    Ellipsis,
    OParen,
    CParen,
    OBrace,
    CBrace,
    OBrack,
    CBrack,
    /// `[*]`.
    Splat,
    /// `${` and `%{`.
    Open,
    /// The `}` that ends one of them.
    End,
    OQuote,
    CQuote,
    OHeredoc,
    /// Template text and heredoc markers: no spaces next to them.
    Lit,
    Strip,
    Equal,
    Minus,
    Bang,
    /// Any other operator or separator, a number or a string.
    Other,
}

impl T {
    fn bracket(self) -> i32 {
        match self {
            T::OParen | T::OBrace | T::OBrack | T::Open => 1,
            T::CParen | T::CBrace | T::CBrack | T::End => -1,
            _ => 0,
        }
    }
}

struct Tok {
    ty: T,
    text: String,
    /// Text written before the token as it stands: the blanks and line breaks of template text.
    raw: String,
    spaces: usize,
    /// A `#` or `//` comment.
    line_comment: bool,
}

impl Tok {
    fn columns(&self) -> usize {
        self.spaces + self.text.chars().count()
    }
}

#[derive(Default)]
struct Line {
    lead: Vec<Tok>,
    assign: Vec<Tok>,
    comment: Option<Tok>,
}

fn columns(toks: &[Tok]) -> usize {
    toks.iter().map(Tok::columns).sum()
}

impl Line {
    fn print(&self, out: &mut String) {
        for t in self.lead.iter().chain(&self.assign).chain(&self.comment) {
            out.extend(std::iter::repeat_n(' ', t.spaces));
            out.push_str(&t.raw);
            out.push_str(&t.text);
        }
    }
}

fn classify(t: &SyntaxToken) -> T {
    let text = t.text();
    match t.kind() {
        k::IDENT | k::BOOL | k::NULL => T::Ident,
        k::KEYWORD => T::Keyword,
        k::TEMPLATE_TEXT | k::HEREDOC_ID => T::Lit,
        k::STRIP => T::Strip,
        k::LEGACY_INDEX => T::Dot,
        k::PUNCT => {
            let in_template = t
                .parent()
                .is_some_and(|p| matches!(p.kind(), k::INTERPOLATION | k::TEMPLATE_DIRECTIVE));
            // The opening quote is the first token of its template; trivia may stand before it.
            let first =
                std::iter::successors(t.prev_sibling_or_token(), |e| e.prev_sibling_or_token())
                    .all(|e| matches!(e.kind(), k::WHITESPACE | k::NEWLINE | k::COMMENT));
            match text {
                "." | ".*" => T::Dot,
                "," => T::Comma,
                "..." => T::Ellipsis,
                "(" => T::OParen,
                ")" => T::CParen,
                "{" => T::OBrace,
                "}" if in_template => T::End,
                "}" => T::CBrace,
                "[" => T::OBrack,
                "]" => T::CBrack,
                "[*]" => T::Splat,
                "${" | "%{" => T::Open,
                "=" => T::Equal,
                "-" => T::Minus,
                "!" => T::Bang,
                "<<" | "<<-" => T::OHeredoc,
                "\"" if first => T::OQuote,
                "\"" => T::CQuote,
                _ => T::Other,
            }
        }
        _ => T::Other,
    }
}

/// Splits the token stream into lines. Blanks and line breaks inside template text stay as written
/// (`raw`); a line break anywhere else ends the line.
fn lines(root: &SyntaxNode) -> Vec<Line> {
    let mut lines = vec![Line::default()];
    // True for each open template, false for each open `${`/`%{`.
    let mut text_mode: Vec<bool> = Vec::new();
    let mut raw = String::new();
    // Templates entered whose first token has not come yet: the trivia before it is the code's.
    let mut opening = 0;
    for event in root.preorder_with_tokens() {
        match event {
            WalkEvent::Enter(SyntaxElement::Node(n))
                if matches!(n.kind(), k::QUOTED_TEMPLATE | k::HEREDOC) =>
            {
                opening += 1;
            }
            WalkEvent::Leave(SyntaxElement::Node(n))
                if matches!(n.kind(), k::QUOTED_TEMPLATE | k::HEREDOC) =>
            {
                text_mode.pop();
                // What is left is the trivia after the closing token, which belongs to the code.
                for _ in raw.matches('\n') {
                    lines.push(Line::default());
                }
                raw.clear();
            }
            WalkEvent::Enter(SyntaxElement::Token(t)) => {
                let text = t.text().replace("\r\n", "\n");
                match t.kind() {
                    k::EOF => {}
                    k::WHITESPACE | k::NEWLINE if text_mode.last() == Some(&true) => {
                        raw.push_str(&text)
                    }
                    k::WHITESPACE => {}
                    k::NEWLINE => lines.extend(text.matches('\n').map(|_| Line::default())),
                    kind => {
                        let ty = classify(&t);
                        match ty {
                            T::Open => text_mode.push(false),
                            T::End => {
                                text_mode.pop();
                            }
                            _ => {}
                        }
                        let line_comment = kind == k::COMMENT && !text.starts_with("/*");
                        let text = if kind == k::COMMENT {
                            text.trim_end().to_string()
                        } else {
                            text
                        };
                        let line = lines.last_mut().expect("a line");
                        line.lead.push(Tok {
                            ty,
                            text,
                            raw: std::mem::take(&mut raw),
                            spaces: 0,
                            line_comment,
                        });
                        if kind != k::COMMENT {
                            text_mode
                                .extend(std::iter::repeat_n(true, std::mem::take(&mut opening)));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    lines
}

/// Whether a `-` after `before` is a sign.
fn unary(before: Option<&Tok>) -> bool {
    let Some(b) = before else { return true };
    match b.ty {
        T::Equal | T::Comma | T::OParen | T::OBrack | T::OBrace | T::Minus | T::Bang => true,
        T::Dot => b.text == ".*",
        T::Other => {
            !b.text.starts_with('"') && b.text.chars().next().is_some_and(|c| !c.is_alphanumeric())
        }
        _ => false,
    }
}

fn space_after(s: &Tok, before: Option<&Tok>, a: &Tok) -> bool {
    let (st, at) = (s.ty, a.ty);
    let ident = matches!(st, T::Ident | T::Keyword);
    if !a.raw.is_empty() {
        return false;
    }
    if ident && at == T::OParen
        || st == T::Dot
        || at == T::Dot
        || at == T::Comma
        || at == T::Ellipsis
    {
        return false;
    }
    if st == T::Comma {
        return true;
    }
    let tight = matches!(st, T::Lit | T::OQuote | T::OHeredoc | T::Strip | T::Bang)
        || matches!(at, T::Lit | T::CQuote | T::Strip | T::End)
        || st == T::End && at == T::Open
        || st == T::Minus && unary(before)
        || matches!(at, T::OBrack | T::Splat)
            && (st == T::Ident || st.bracket() < 0 && st != T::End);
    if tight {
        return false;
    }
    match (st, at) {
        (T::Open, _) => at == T::OBrace,
        (T::OBrace, _) => at != T::CBrace,
        (_, T::CBrace) => true,
        _ => st.bracket() <= 0 && at.bracket() >= 0,
    }
}

fn space_cell(cell: &mut [Tok]) {
    for i in 1..cell.len() {
        let (head, tail) = cell.split_at_mut(i);
        let before = i.checked_sub(2).map(|b| &head[b]);
        tail[0].spaces = usize::from(space_after(&head[i - 1], before, &tail[0]));
    }
}

fn indent(lines: &mut [Line], size: usize) {
    split_cells(lines);
    // Net number of brackets each open line left, innermost last.
    let mut stack: Vec<i32> = Vec::new();
    for line in lines.iter_mut() {
        if line.lead.is_empty() {
            continue;
        }
        let net: i32 = line
            .lead
            .iter()
            .chain(&line.assign)
            .map(|t| t.ty.bracket())
            .sum();
        let depth = if net > 0 {
            let depth = stack.len();
            stack.push(net);
            depth
        } else {
            let mut closed = -net;
            while closed > 0
                && let Some(top) = stack.last_mut()
            {
                if closed >= *top {
                    closed -= *top;
                    stack.pop();
                } else {
                    *top -= closed;
                    closed = 0;
                }
            }
            stack.len()
        };
        space_cell(&mut line.lead);
        if let Some(first) = line.lead.first_mut() {
            first.spaces = depth * size;
        }
        if let Some(eq) = line.assign.first_mut() {
            eq.spaces = 1;
        }
        space_cell(&mut line.assign);
    }
    align(lines);
}

/// The `=` and what follows it become the assign cell of a line that holds a whole expression, a
/// trailing `#` comment the comment cell.
fn split_cells(lines: &mut [Line]) {
    for line in lines {
        if line.lead.len() > 1 && line.lead.last().is_some_and(|t| t.line_comment) {
            line.comment = line.lead.pop();
        }
        let Some(eq) = line
            .lead
            .iter()
            .skip(1)
            .position(|t| t.ty == T::Equal)
            .map(|i| i + 1)
        else {
            continue;
        };
        if line.lead[eq..].iter().map(|t| t.ty.bracket()).sum::<i32>() == 0 {
            line.assign = line.lead.split_off(eq);
        }
    }
}

/// Runs of consecutive lines with an assign cell: the `=` lines up one space after the widest key.
/// Runs of consecutive lines with a comment: the comments line up one space after the longest code.
fn align(lines: &mut [Line]) {
    for_runs(
        lines,
        |l| !l.assign.is_empty(),
        |run| {
            let widest = run.iter().map(|l| columns(&l.lead)).max().unwrap_or(0);
            for l in run {
                l.assign[0].spaces = widest - columns(&l.lead) + 1;
            }
        },
    );
    for_runs(
        lines,
        |l| l.comment.is_some(),
        |run| {
            let widest = run
                .iter()
                .map(|l| columns(&l.lead) + columns(&l.assign))
                .max()
                .unwrap_or(0);
            for l in run {
                let own = columns(&l.lead) + columns(&l.assign);
                if let Some(c) = &mut l.comment {
                    c.spaces = widest - own + 1;
                }
            }
        },
    );
}

fn for_runs(lines: &mut [Line], member: impl Fn(&Line) -> bool, mut each: impl FnMut(&mut [Line])) {
    let mut start = None;
    for i in 0..=lines.len() {
        let inside = lines.get(i).is_some_and(&member);
        match (start, inside) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                each(&mut lines[s..i]);
                start = None;
            }
            _ => {}
        }
    }
}
