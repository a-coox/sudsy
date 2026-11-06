//! PowerShell layout after PSScriptAnalyzer's `Invoke-Formatter` (its CodeFormatting settings):
//! every line is re-indented, and blanks and line breaks are added where it adds them (braces,
//! operators, separators, pipes, `=` of a hashtable). Blanks the source has are kept, and so are
//! the words, strings and line breaks the formatter has no rule for. The tree is flat, so the
//! formatter walks the tokens.

use crate::generated::kind as k;
use core_config::{OptionDef, OptionSchema, shared};
use core_format::comments::{self, Comment, CommentStyle};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken};
use std::collections::HashMap;

pub const MAX_BLANK_LINES: &str = "max_blank_lines";

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::indent_size(4),
        OptionDef::choice(
            shared::INDENT_STYLE,
            &["space", "tab"],
            "space",
            "Indent with spaces or tabs.",
        ),
        shared::final_newline(true),
        shared::end_of_line(),
        OptionDef::int(
            MAX_BLANK_LINES,
            10,
            (0, 10),
            "Largest number of blank lines kept between statements and comments.",
        ),
    ])
}

pub struct PowerShellFormatter;

impl NodeFormatter for PowerShellFormatter {
    fn comment_style(&self, _: &core_config::ResolvedOptions) -> CommentStyle {
        // The blanks before a comment are the source's, none added.
        CommentStyle {
            gap: 0,
            keep_gap: true,
            ..Default::default()
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok((node.kind() == k::PROGRAM).then(|| program(cx, node)))
    }
}

/// A token with what the source has around it.
struct Tk {
    tok: SyntaxToken,
    kind: SyntaxKind,
    /// Source lines of the token's first and last character.
    first_line: usize,
    last_line: usize,
    /// Line breaks between the previous token and this one.
    breaks: usize,
    /// A backtick and a line break come before the token.
    cont: bool,
    /// The blanks before the token on its line (a comment between counts as one blank).
    gap: String,
    /// The blanks in front of the backtick.
    cont_gap: String,
    /// The token starts an output line.
    newline: bool,
    /// The comments on lines of their own in front of the token.
    leading: Vec<Comment>,
    /// A comment follows the token on its line.
    trailing: bool,
    /// The matching bracket of an opener or closer.
    pair: Option<usize>,
}

impl Tk {
    fn text(&self) -> &str {
        self.tok.text()
    }
    fn is(&self, kinds: &[SyntaxKind]) -> bool {
        kinds.contains(&self.kind)
    }
    /// Is it one of these keywords (PowerShell ignores their case)?
    fn keyword(&self, words: &[&str]) -> bool {
        self.kind == k::KEYWORD && words.iter().any(|w| self.text().eq_ignore_ascii_case(w))
    }
}

fn is_open(kind: SyntaxKind) -> bool {
    matches!(kind, k::OPEN_BRACE | k::HASH_OPEN | k::OPEN_PAREN)
}

fn is_close(kind: SyntaxKind) -> bool {
    matches!(kind, k::CLOSE_BRACE | k::CLOSE_PAREN)
}

fn collect(g: &Grammar, root: &SyntaxNode) -> Vec<Tk> {
    let mut line = 0;
    let mut out: Vec<Tk> = Vec::new();
    for t in root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let n = comments::count_breaks(t.text());
        if !g.is_trivia(t.kind()) {
            let mut tk = Tk {
                kind: t.kind(),
                first_line: line,
                last_line: line + n,
                breaks: 0,
                cont: false,
                gap: String::new(),
                cont_gap: String::new(),
                newline: false,
                leading: comments::leading_comments(g, &t),
                trailing: comments::has_trailing_comment(g, &t),
                pair: None,
                tok: t.clone(),
            };
            gap(g, &t, &mut tk);
            out.push(tk);
        }
        line += n;
    }
    out
}

/// Fills in what the trivia between the previous token and `t` say.
fn gap(g: &Grammar, t: &SyntaxToken, tk: &mut Tk) {
    let mut trivia = Vec::new();
    let mut cur = t.prev_token();
    while let Some(p) = cur.filter(|p| g.is_trivia(p.kind())) {
        cur = p.prev_token();
        trivia.push(p);
    }
    let (mut commented, mut blanks) = (false, false);
    for p in trivia.iter().rev() {
        match p.kind() {
            k::NEWLINE => {
                tk.breaks += comments::count_breaks(p.text());
                tk.gap.clear();
            }
            k::WHITESPACE if p.text().contains('`') => {
                tk.cont = true;
                tk.cont_gap = std::mem::take(&mut tk.gap);
            }
            k::WHITESPACE => {
                tk.gap.push_str(p.text());
                blanks = true;
            }
            _ => {
                commented = true;
                tk.gap.clear();
            }
        }
    }
    if commented && tk.breaks == 0 {
        tk.gap = if blanks { " " } else { "" }.into();
    }
}

/// Pairs each opener with its closer (a mismatch is a parse error and cannot get here).
fn pair_brackets(tks: &mut [Tk]) {
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..tks.len() {
        if is_open(tks[i].kind) {
            stack.push(i);
        } else if is_close(tks[i].kind)
            && let Some(o) = stack.pop()
        {
            tks[o].pair = Some(i);
            tks[i].pair = Some(o);
        }
    }
}

/// Is the bracket pair of token `i` spread over several source lines?
fn multiline(tks: &[Tk], i: usize) -> bool {
    tks[i].pair.is_some_and(|p| {
        let (a, b) = (i.min(p), i.max(p));
        tks[a].last_line != tks[b].first_line
    })
}

/// Is token `i` directly inside the body of a `class` or `switch` (`which`)?
fn in_block(tks: &[Tk], i: usize, which: &str) -> bool {
    let mut j = i;
    let open = loop {
        if j == 0 {
            return false;
        }
        j -= 1;
        match tks[j].kind {
            k::CLOSE_BRACE | k::CLOSE_PAREN => match tks[j].pair {
                Some(o) => j = o,
                None => return false,
            },
            k::OPEN_BRACE => break j,
            _ if is_open(tks[j].kind) => return false,
            _ => {}
        }
    };
    // `class A : B {` and `switch -regex ($x) {`
    tks[open.saturating_sub(6)..open]
        .iter()
        .any(|t| t.keyword(&[which]))
}

/// Is the `{` at `o` the block of a statement (`if (...) {`, `else {`, `function f {`,
/// `catch [T] {`) and not a script block that is an expression or an argument
/// (`ForEach-Object {`, `$x = {`)?
fn statement_block(tks: &[Tk], o: usize) -> bool {
    if tks[o].kind != k::OPEN_BRACE || o == 0 {
        return false;
    }
    let prev = &tks[o - 1];
    match prev.kind {
        k::KEYWORD => prev.keyword(&[
            "else",
            "catch",
            "try",
            "finally",
            "do",
            "begin",
            "process",
            "end",
            "dynamicparam",
            "default",
        ]),
        // The condition of a control statement, the parameters of a function or the method of a
        // class (not `param(...)`).
        k::CLOSE_PAREN => prev.pair.is_some_and(|p| {
            p > 0
                && (tks[p - 1].keyword(&[
                    "if", "elseif", "while", "switch", "foreach", "for", "until", "catch",
                ]) || (p > 1
                    && tks[p - 1].kind == k::WORD
                    && (tks[p - 2].keyword(&["function", "filter"]) || in_block(tks, p, "class"))))
        }),
        // `catch [System.Exception], [System.IO.IOException] {`
        k::CLOSE_BRACKET => (tks[..o].iter().rev())
            .find(|t| !t.is(&[k::WORD, k::OPEN_BRACKET, k::CLOSE_BRACKET, k::COMMA]))
            .is_some_and(|t| t.keyword(&["catch"])),
        k::WORD => {
            (o > 1 && tks[o - 2].keyword(&["function", "filter", "class", "enum"]))
                || in_block(tks, o, "switch")
        }
        // The clause of a `switch`: `5 {`.
        k::STRING => in_block(tks, o, "switch"),
        _ => false,
    }
}

/// Is token `i` a bracket of a block that spans lines and whose braces the formatter places: a
/// statement block or a hashtable?
fn placed_block(tks: &[Tk], i: usize) -> bool {
    let Some(p) = tks[i].pair else {
        return false;
    };
    let o = i.min(p);
    multiline(tks, o) && (tks[o].kind == k::HASH_OPEN || statement_block(tks, o))
}

/// Where the output has line breaks: the source's, minus a brace that moves up to its head, plus
/// breaks after `{`, before `}` and before `else`/`catch`/`finally` of a block that spans lines.
fn place_breaks(tks: &mut [Tk]) {
    let mut joined = Vec::new();
    for i in 0..tks.len() {
        let mut nl = i > 0 && tks[i].breaks > 0;
        if i > 0 && !tks[i].cont {
            let prev = &tks[i - 1];
            match tks[i].kind {
                // `)` + line break + `{`: the brace joins the line of its head.
                k::OPEN_BRACE
                    if nl
                        && statement_block(tks, i)
                        && tks[i].leading.is_empty()
                        && !prev.trailing
                        && multiline(tks, i) =>
                {
                    nl = false;
                    joined.push(i);
                }
                // `{ a` + line break + `}`: the statements go to a line of their own.
                k::CLOSE_BRACE if !nl && placed_block(tks, i) && !prev.is(&[k::OPEN_BRACE]) => {
                    nl = true;
                }
                k::KEYWORD
                    if !nl
                        && tks[i].keyword(&["else", "elseif", "catch", "finally"])
                        && prev.kind == k::CLOSE_BRACE
                        && placed_block(tks, i - 1) =>
                {
                    nl = true;
                }
                _ => {}
            }
            if !nl
                && prev.kind == k::OPEN_BRACE
                && multiline(tks, i - 1)
                && statement_block(tks, i - 1)
            {
                nl = !tks[i].is(&[k::CLOSE_BRACE]);
            }
        }
        tks[i].newline = nl || tks[i].cont;
    }
    for i in joined {
        tks[i].gap.clear();
    }
}

/// For each `=` of a multi-line hashtable entry that starts its line: the blanks that line up
/// the `=` of all such entries.
fn align_hashtables(cx: &FormatCtx<'_>, root: &SyntaxNode, tks: &[Tk]) -> HashMap<usize, usize> {
    let by_start: HashMap<usize, usize> = (tks.iter().enumerate())
        .map(|(i, t)| (usize::from(t.tok.text_range().start()), i))
        .collect();
    let mut pads = HashMap::new();
    for hash in root.descendants().filter(|n| n.kind() == k::HASH) {
        let Some(open) = cx.first_token(&hash) else {
            continue;
        };
        let Some(&oi) = by_start.get(&usize::from(open.text_range().start())) else {
            continue;
        };
        if !multiline(tks, oi) {
            continue;
        }
        // (key width, index of `=`) of the entries that start a line and have `=` on it.
        let mut entries = Vec::new();
        for entry in hash.children().filter(|n| n.kind() == k::HASH_ENTRY) {
            let (Some(first), Some(eq)) = (
                cx.first_token(&entry),
                entry
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| t.kind() == k::ASSIGN),
            ) else {
                continue;
            };
            let (Some(&fi), Some(&ei)) = (
                by_start.get(&usize::from(first.text_range().start())),
                by_start.get(&usize::from(eq.text_range().start())),
            ) else {
                continue;
            };
            if !tks[fi].newline || tks[fi].first_line != tks[ei].first_line {
                continue;
            }
            let end = usize::from(tks[ei - 1].tok.text_range().end());
            let width = end - usize::from(first.text_range().start());
            entries.push((width, ei));
        }
        let widest = entries.iter().map(|e| e.0).max().unwrap_or(0);
        for (width, ei) in entries {
            pads.insert(ei, widest - width);
        }
    }
    pads
}

/// How the blanks between `prev` and `cur` (on one line) are set: `None` keeps the source's,
/// `Some(false)` adds one where there is none, `Some(true)` makes it exactly one.
fn blank(tks: &[Tk], i: usize, pads: &HashMap<usize, usize>) -> Option<bool> {
    let (prev, cur) = (&tks[i - 1], &tks[i]);
    let binary = |t: &Tk| {
        t.kind == k::OP && t.text() != ".." && t.tok.parent().is_some_and(|p| p.kind() == k::BINARY)
    };
    let one = match (prev.kind, cur.kind) {
        (k::PIPE, _) | (_, k::PIPE) => return Some(false),
        // Invoke-Formatter does not add the blank after the aligned `=` of `a=1`.
        (k::ASSIGN, _) if pads.contains_key(&(i - 1)) && cur.gap.is_empty() => return None,
        (k::COMMA | k::SEMI, _) | (k::ASSIGN, _) | (_, k::ASSIGN) => true,
        _ if binary(prev) || binary(cur) => true,
        (k::OPEN_BRACE, k::CLOSE_BRACE) => false,
        (k::OPEN_BRACE, _) => true,
        // `@{ a = 1 }`: the blank before `}` only; `{ a }` has both.
        (_, k::CLOSE_BRACE) => !is_open(prev.kind),
        (_, k::OPEN_BRACE) => {
            // `$x.ForEach{ }`: the block is the argument of the method.
            let method =
                prev.kind == k::WORD && (prev.text().contains('.') || prev.text().contains("::"));
            !is_open(prev.kind) && prev.kind != k::OPEN_BRACKET && !method
        }
        (k::KEYWORD, k::OPEN_PAREN) => {
            cur.text() == "("
                && prev.keyword(&["if", "elseif", "while", "switch", "foreach", "for", "until"])
        }
        _ => false,
    };
    one.then_some(true)
}

enum Frame {
    /// An open bracket.
    Open,
    /// The lines that continue a pipeline (`|` at the end of a line) or a statement (a backtick
    /// at the end of a line); a line that does both is indented twice.
    Continuation { pipe: bool, tick: bool },
}

/// The indentation levels the open frames add up to.
fn depth(frames: &[Frame]) -> usize {
    (frames.iter())
        .map(|f| match f {
            Frame::Open => 1,
            Frame::Continuation { pipe, tick } => usize::from(*pipe) + usize::from(*tick),
        })
        .sum()
}

/// Ends the continuation lines and the bracket of a closer.
fn close(frames: &mut Vec<Frame>) {
    while matches!(frames.last(), Some(Frame::Continuation { .. })) {
        frames.pop();
    }
    frames.pop();
}

fn program(cx: &FormatCtx<'_>, root: &SyntaxNode) -> Doc {
    let g = cx.grammar;
    let max_blank = cx.options.int(MAX_BLANK_LINES) as usize;
    let mut tks = collect(g, root);
    pair_brackets(&mut tks);
    place_breaks(&mut tks);
    let pads = align_hashtables(cx, root, &tks);

    let mut lines: Vec<Doc> = Vec::new();
    // The current line's parts, its indentation level and whether it starts with a line break.
    let mut cur: Vec<Doc> = Vec::new();
    let mut level = 0;
    let mut frames: Vec<Frame> = Vec::new();
    let flush = |lines: &mut Vec<Doc>, cur: &mut Vec<Doc>, level: usize| {
        if cur.is_empty() {
            return;
        }
        let mut d = doc::concat(std::mem::take(cur));
        for _ in 0..level {
            d = doc::indent(d);
        }
        lines.push(d);
    };
    let blank_lines = |n: usize| (0..n.min(max_blank)).map(|_| doc::hardline());

    for i in 0..tks.len() {
        let is_eof = tks[i].kind == k::EOF;
        if is_eof && tks[i].leading.is_empty() {
            continue;
        }
        let starts = i > 0 && (tks[i].newline || is_eof);
        if starts {
            if tks[i].cont {
                cur.push(doc::text(format!("{}`", tks[i].cont_gap)));
            }
            flush(&mut lines, &mut cur, level);
            // A pipeline or backtick line continues on the next line.
            let (pipe, tick) = (tks[i - 1].kind == k::PIPE, tks[i].cont);
            match frames.last_mut() {
                Some(Frame::Continuation { pipe: p, tick: t }) if pipe || tick => {
                    *p |= pipe;
                    *t |= tick;
                }
                Some(Frame::Continuation { .. }) => {
                    frames.pop();
                }
                _ if pipe || tick => frames.push(Frame::Continuation { pipe, tick }),
                _ => {}
            }
            // Comments stand where the code in front of the closing bracket stands.
            let mut blank = if tks[i].breaks > 0 {
                comments::blank_lines_before(g, &tks[i].tok)
            } else {
                0
            };
            let leading = std::mem::take(&mut tks[i].leading);
            let mut inline: Vec<Doc> = Vec::new();
            for c in &leading {
                if c.kind == comments::CommentKind::Block && c.breaks_after == 0 {
                    inline.push(core_format::context::comment_doc(c));
                    inline.push(doc::lit(" "));
                    continue;
                }
                let mut parts: Vec<Doc> = blank_lines(blank).collect();
                parts.extend([doc::hardline(), core_format::context::comment_doc(c)]);
                lines.push(indented(depth(&frames), doc::concat(parts)));
                blank = c.breaks_after.saturating_sub(1);
            }
            if is_close(tks[i].kind) {
                close(&mut frames);
            }
            level = depth(&frames);
            cur.extend(blank_lines(blank));
            cur.push(doc::hardline());
            cur.extend(inline);
        } else if i > 0 {
            let gap = spacing(&tks, i, &pads);
            if !gap.is_empty() {
                cur.push(doc::text(gap));
            }
            if is_close(tks[i].kind) {
                close(&mut frames);
            }
        } else {
            // The first token: its comments stand on lines of their own.
            let leading = std::mem::take(&mut tks[i].leading);
            for c in &leading {
                cur.push(core_format::context::comment_doc(c));
                cur.push(doc::hardline());
                cur.extend(blank_lines(c.breaks_after.saturating_sub(1)));
            }
        }
        if !is_eof {
            cur.push(cx.token_text(&tks[i].tok));
            cur.push(cx.trailing_comments_doc(&tks[i].tok));
        }
        if is_open(tks[i].kind) {
            frames.push(Frame::Open);
        }
    }
    flush(&mut lines, &mut cur, level);
    doc::concat(lines)
}

fn indented(levels: usize, mut d: Doc) -> Doc {
    for _ in 0..levels {
        d = doc::indent(d);
    }
    d
}

/// The blanks between token `i - 1` and `i` on one line.
fn spacing(tks: &[Tk], i: usize, pads: &HashMap<usize, usize>) -> String {
    if let Some(pad) = pads.get(&i) {
        return " ".repeat(pad + 1);
    }
    match blank(tks, i, pads) {
        Some(true) => " ".into(),
        Some(false) if tks[i].gap.is_empty() => " ".into(),
        _ => tks[i].gap.clone(),
    }
}
