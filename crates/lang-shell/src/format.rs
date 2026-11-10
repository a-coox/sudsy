//! Shell formatting after shfmt's default layout: one statement per line, bodies indented one
//! level, `then`/`do` on the line of their head, case patterns at the `case` column and their
//! bodies indented. A compound command written on one line stays on one line. Words, strings,
//! expansions, substitutions and tests are kept as written; only the layout between them changes.

use crate::generated::kind as k;
use core_config::{OptionDef, OptionSchema, Preset, shared};
use core_format::shapes::dangling_comments;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::indent_size(4),
        OptionDef::choice(
            shared::INDENT_STYLE,
            &["space", "tab"],
            "tab",
            "Indent with spaces or tabs.",
        ),
        shared::final_newline(true),
        shared::end_of_line(),
        shared::max_blank_lines(
            1,
            "Largest number of blank lines kept between statements and comments.",
        ),
    ])
    .with_presets(vec![Preset::new(
        "shell/shellcheck",
        include_str!("../presets/shellcheck.yaml"),
    )])
}

pub struct ShellFormatter;

impl NodeFormatter for ShellFormatter {
    /// Redirect operators lose the blanks after them, nothing else changes.
    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        if restyled(before) == restyled(after) {
            Ok(())
        } else {
            Err("a redirection or array changed".into())
        }
    }

    fn comment_style(
        &self,
        _: &core_config::ResolvedOptions,
    ) -> core_format::comments::CommentStyle {
        core_format::comments::CommentStyle {
            keep_gap: true,
            ..Default::default()
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::PROGRAM => program(cx, node)?,
            k::COMMAND | k::PIPELINE | k::LIST | k::REDIRECTED | k::NEGATED => flow(cx, node)?,
            k::IF
            | k::ELIF
            | k::ELSE
            | k::DO_GROUP
            | k::GROUP
            | k::SUBSHELL
            | k::WHILE
            | k::FOR
            | k::C_FOR => compound(cx, node)?,
            k::CASE => case(cx, node)?,
            k::CASE_ITEM => case_item(cx, node)?,
            k::FUNCTION => function(cx, node)?,
            k::HEREDOC_REDIRECT => heredoc(cx, node)?,
            k::ASSIGN => assign(cx, node),
            k::TEST | k::STRING | k::CONCAT | k::SUBST => cx.verbatim_node(node),
            _ => return Ok(None),
        }))
    }
}

/// The redirections without blanks and the arrays without padding, as the formatter writes them.
fn restyled(tree: &SyntaxNode) -> Vec<String> {
    (tree.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .filter_map(|t| match t.kind() {
            k::REDIRECT => Some(t.text().split_whitespace().collect()),
            k::ARRAY => Some(unpadded(t.text())),
            _ => None,
        })
        .collect()
}

/// `( b c )` is `(b c)`.
fn unpadded(array: &str) -> String {
    match array.strip_prefix('(').and_then(|a| a.strip_suffix(')')) {
        Some(inner) => format!("({})", inner.trim_matches([' ', '\t'])),
        None => array.to_string(),
    }
}

/// A significant child; `cont` when a line continuation (`\` and a line break) precedes it.
struct E {
    el: SyntaxElement,
    cont: bool,
}

impl E {
    fn tok(&self) -> Option<&SyntaxToken> {
        self.el.as_token()
    }
    /// The text of a keyword token.
    fn kw(&self) -> Option<&str> {
        self.tok()
            .filter(|t| t.kind() == k::KEYWORD)
            .map(|t| t.text())
    }
    /// The text of an operator token.
    fn op(&self) -> Option<&str> {
        self.tok().filter(|t| t.kind() == k::OP).map(|t| t.text())
    }
    fn is_kw(&self, words: &[&str]) -> bool {
        self.kw().is_some_and(|w| words.contains(&w))
    }
    fn is(&self, kind: core_syntax::SyntaxKind) -> bool {
        self.el.kind() == kind
    }
    fn first(&self, cx: &FormatCtx<'_>) -> Option<SyntaxToken> {
        match &self.el {
            SyntaxElement::Token(t) => Some(t.clone()),
            SyntaxElement::Node(n) => cx.first_token(n),
        }
    }
    fn last(&self, cx: &FormatCtx<'_>) -> Option<SyntaxToken> {
        match &self.el {
            SyntaxElement::Token(t) => Some(t.clone()),
            SyntaxElement::Node(n) => cx.last_token(n),
        }
    }
    fn doc(&self, cx: &FormatCtx<'_>) -> Result<Doc, FormatError> {
        match &self.el {
            SyntaxElement::Token(t) if t.kind() == k::REDIRECT => Ok(doc::concat(vec![
                cx.leading_comments_doc(t),
                core_format::context::verbatim_text(&redirect_text(t.text())),
                cx.trailing_comments_doc(t),
            ])),
            SyntaxElement::Token(t) => Ok(cx.token(t)),
            SyntaxElement::Node(n) => cx.format(n),
        }
    }
}

/// An assignment as written, except that `a=( b )` is `a=(b)`.
fn assign(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let (Some(first), Some(last)) = (cx.first_token(node), cx.last_token(node)) else {
        return Doc::Nil;
    };
    // The node's trailing trivia are not part of the assignment.
    let start = node.text_range().start();
    let range = TextRange::new(
        first.text_range().start() - start,
        last.text_range().end() - start,
    );
    let text = node.text().slice(range).to_string();
    if text.contains('\n') {
        return cx.verbatim_node(node);
    }
    doc::concat(vec![
        cx.leading_comments_doc(&first),
        doc::text(assign_text(&text)),
        cx.trailing_comments_doc(&last),
    ])
}

/// `a=( b c )` is `a=(b c)`.
fn assign_text(s: &str) -> String {
    let Some(open) = s.find("=(") else {
        return s.to_string();
    };
    let name = s[..open].trim_end_matches('+');
    let inner = s[open + 2..]
        .strip_suffix(')')
        .map(|i| i.trim_matches([' ', '\t']));
    match inner {
        Some(inner)
            if !inner.ends_with('\\')
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') =>
        {
            format!("{}=({inner})", &s[..open])
        }
        _ => s.to_string(),
    }
}

/// `2> file` is `2>file`: no blanks between the operator and its word, except where the word
/// would run into the operator (`< <(cmd)`, `> -x`).
fn redirect_text(s: &str) -> String {
    let Some(at) = s.find(['<', '>', '&']) else {
        return s.to_string();
    };
    let op_end = at
        + s[at..]
            .find(|c| !"<>&|-".contains(c))
            .unwrap_or(s.len() - at);
    let rest = s[op_end..].trim_start_matches([' ', '\t']);
    if rest.len() == s.len() - op_end || rest.starts_with(['<', '>', '-', '(']) {
        return s.to_string();
    }
    format!("{}{rest}", &s[..op_end])
}

/// The significant children; `cont` when a line continuation precedes the child.
fn elems(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<E> {
    node.children_with_tokens()
        .filter(|el| !matches!(el, SyntaxElement::Token(t) if cx.grammar.is_trivia(t.kind())))
        .map(|el| {
            let first = match &el {
                SyntaxElement::Token(t) => Some(t.clone()),
                SyntaxElement::Node(n) => cx.first_token(n),
            };
            let cont = first.is_some_and(|t| continued_before(cx, &t));
            E { el, cont }
        })
        .collect()
}

/// Does a line continuation (`\` and a line break, whitespace trivia) stand right before `t`?
fn continued_before(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    let mut cur = t.prev_token();
    while let Some(p) = cur.filter(|p| cx.grammar.is_trivia(p.kind())) {
        if p.kind() == k::WHITESPACE && p.text().contains('\\') {
            return true;
        }
        cur = p.prev_token();
    }
    false
}

/// Is there a line break (not a continuation) between `t` and the next significant token?
fn breaks(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    let mut cur = t.next_token();
    while let Some(n) = cur.filter(|n| cx.grammar.is_trivia(n.kind())) {
        if n.kind() != k::WHITESPACE {
            return true;
        }
        cur = n.next_token();
    }
    false
}

/// The column of a token (bytes since the start of its line).
fn column(t: &SyntaxToken) -> usize {
    let mut col = 0;
    let mut prev = t.prev_token();
    while let Some(p) = prev {
        let text = p.text();
        if let Some(i) = text.rfind('\n') {
            return col + text.len() - i - 1;
        }
        col += text.len();
        prev = p.prev_token();
    }
    col
}

/// How many of the comments in front of a keyword that ends a body stand deeper than the
/// keyword: those belong to the body, the others to the keyword.
fn deeper_comments(cx: &FormatCtx<'_>, t: &SyntaxToken) -> usize {
    let at = column(t);
    comments::leading_comments(cx.grammar, t)
        .iter()
        .take_while(|c| column(c.token()) > at)
        .count()
}

/// A keyword that ends a body (`fi`, `}`, `;;`, `else`): with the comments in front of it that
/// do not belong to the body (see [`dangling`]) and its trailing comments.
fn kw(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    let cs = comments::leading_comments(cx.grammar, t);
    doc::concat(vec![
        core_format::context::leading_comments_doc(&cs[deeper_comments(cx, t)..]),
        cx.token_text(t),
        cx.trailing_comments_doc(t),
    ])
}

fn max_blank(cx: &FormatCtx<'_>) -> usize {
    cx.options.max_blank_lines()
}

fn next_significant(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Option<SyntaxToken> {
    let mut cur = t.next_token();
    while let Some(n) = cur {
        if !cx.grammar.is_trivia(n.kind()) {
            return Some(n);
        }
        cur = n.next_token();
    }
    None
}

/// Does the node span a line break, from its first to its last token?
fn is_block(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let mut n = node.clone();
    while matches!(n.kind(), k::ELIF | k::ELSE | k::DO_GROUP) {
        match n.parent() {
            Some(p) => n = p,
            None => break,
        }
    }
    let lines = match (cx.first_token(&n), cx.last_token(&n)) {
        (Some(a), Some(b)) => cx.has_line_break(
            &n,
            TextRange::new(a.text_range().start(), b.text_range().end()),
        ),
        _ => false,
    };
    lines || multi_statement_body(cx, &n)
}

/// Does a compound command at or below `node` have a body of several statements? One inside a
/// substitution does not count: its text is kept as written.
fn multi_statement_body(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    statements_in_body(cx, node) > 1
        || (node.children())
            .filter(|c| c.kind() != k::SUBST)
            .any(|c| multi_statement_body(cx, &c))
}

/// The statements in the body of a compound command or case item (0 for other nodes).
fn statements_in_body(cx: &FormatCtx<'_>, node: &SyntaxNode) -> usize {
    let opener: &[&str] = match node.kind() {
        k::IF | k::ELIF => &["then"],
        k::ELSE | k::DO_GROUP => &["else", "do"],
        k::GROUP => &["{"],
        k::SUBSHELL => &["("],
        k::CASE_ITEM => &[")"],
        _ => return 0,
    };
    let es = elems(cx, node);
    let Some(oi) = es.iter().position(|e| e.is_kw(opener)) else {
        return 0;
    };
    (es[oi + 1..])
        .iter()
        .take_while(|e| !(e.is(k::ELIF) || e.is(k::ELSE)))
        .filter(|e| e.el.as_node().is_some())
        .count()
}

fn program(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es: Vec<E> = elems(cx, node)
        .into_iter()
        .filter(|e| !e.is(k::EOF))
        .collect();
    let mut parts = vec![block_list(cx, &es)?];
    let eof = node
        .last_token()
        .ok_or_else(|| FormatError::Shape("Program without tokens".into()))?;
    let trailing = comments::leading_comments(cx.grammar, &eof);
    if !trailing.is_empty() {
        if !es.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(core_format::context::leading_comments_doc(&trailing));
    }
    Ok(doc::concat(parts))
}

/// Statements one per line, with the source's blank lines (up to `max_blank_lines`); `;` goes,
/// `&` stays after its statement.
fn block_list(cx: &FormatCtx<'_>, es: &[E]) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let max = max_blank(cx);
    let mut parts = Vec::new();
    let mut first_stmt = true;
    for e in es {
        match e.tok() {
            Some(t) if t.kind() == k::SEMI => parts.push(cx.trailing_comments_doc(t)),
            Some(t) if e.op() == Some("&") => parts.extend([doc::lit(" "), cx.token(t)]),
            _ => {
                if !first_stmt {
                    parts.push(doc::hardline());
                    if let Some(first) = e.first(cx) {
                        let blank = comments::blank_lines_before(g, &first).min(max);
                        parts.extend((0..blank).map(|_| doc::hardline()));
                    }
                }
                first_stmt = false;
                parts.push(e.doc(cx)?);
            }
        }
    }
    Ok(doc::concat(parts))
}

/// Statements on one line: `a; b`, `a & b`; with `terminate` a `;` ends the last one.
fn inline_list(cx: &FormatCtx<'_>, es: &[E], terminate: bool) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    let (mut any, mut amp) = (false, false);
    for e in es {
        match e.tok() {
            Some(t) if t.kind() == k::SEMI => parts.push(cx.trailing_comments_doc(t)),
            Some(t) if e.op() == Some("&") => {
                parts.extend([doc::lit(" "), cx.token(t)]);
                amp = true;
            }
            _ => {
                if any {
                    // No blank before leading comments: their line break would put it ahead of
                    // the previous statement's trailing comment, one more blank per pass.
                    let comments_follow = e
                        .first(cx)
                        .is_some_and(|t| !comments::leading_comments(cx.grammar, &t).is_empty());
                    parts.push(doc::lit(match (amp, comments_follow) {
                        (true, _) => " ",
                        (false, true) => ";",
                        (false, false) => "; ",
                    }));
                }
                parts.push(e.doc(cx)?);
                any = true;
                amp = false;
            }
        }
    }
    if terminate && any && !amp {
        parts.push(doc::lit(";"));
    }
    Ok(doc::concat(parts))
}

/// Elements joined by a space, with the layout of `a &&` + line break + `b` kept (the rest
/// indented one level) and line continuations kept.
fn flow(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    let mut head = Vec::new();
    let mut tail: Option<Vec<Doc>> = None;
    for (i, e) in es.iter().enumerate() {
        let d = e.doc(cx)?;
        if i == 0 {
            head.push(d);
            continue;
        }
        let prev = &es[i - 1];
        let after_op = matches!(prev.op(), Some("&&" | "||" | "|" | "|&"));
        let broken_after_op = after_op && (prev.cont || prev.tok().is_some_and(|t| breaks(cx, t)));
        let before_op = matches!(e.op(), Some("&&" | "||" | "|" | "|&"));
        let glued = !e.cont
            && !broken_after_op
            && !after_op
            && !before_op
            && !e.is(k::REDIRECT)
            && !e.is(k::HEREDOC_REDIRECT)
            && prev.last(cx).and_then(|t| t.next_token()) == e.first(cx);
        let sep = if glued {
            vec![]
        } else if e.cont && before_op {
            vec![doc::lit(" ")]
        } else if e.cont {
            vec![doc::lit(" \\"), doc::hardline()]
        } else if broken_after_op {
            vec![doc::hardline()]
        } else {
            vec![doc::lit(" ")]
        };
        if ((e.cont && !before_op) || broken_after_op) && tail.is_none() {
            tail = Some(Vec::new());
        }
        let target = tail.as_mut().unwrap_or(&mut head);
        target.extend(sep);
        target.push(d);
    }
    if let Some(tail) = tail {
        head.push(doc::indent(doc::concat(tail)));
    }
    Ok(doc::concat(head))
}

/// `<<EOF` and what follows it on the line; the body and the delimiter line are printed at the
/// next line break, at column 0.
fn heredoc(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    let mut glue = true;
    let mut digits = false;
    for e in elems(cx, node) {
        if e.is(k::HEREDOC_BODY) || e.is(k::HEREDOC_END) {
            continue;
        }
        // `3<<EOF`: the descriptor and the operator, and the operator and the delimiter, touch.
        if !glue && !(digits && e.is(k::HEREDOC_OP)) {
            parts.push(doc::lit(" "));
        }
        glue = e.is(k::HEREDOC_OP);
        digits = e
            .tok()
            .is_some_and(|t| t.text().bytes().all(|b| b.is_ascii_digit()));
        parts.push(e.doc(cx)?);
    }
    // The body starts on the line after the last element, with its indentation (blanks that
    // are content), and runs to the end of the delimiter line.
    let kids: Vec<SyntaxElement> = node.children_with_tokens().collect();
    let is_body = |e: &SyntaxElement| matches!(e.kind(), k::HEREDOC_BODY | k::HEREDOC_END);
    let first = kids.iter().position(is_body).unwrap_or(kids.len());
    let last = kids.iter().rposition(is_body).unwrap_or(0);
    let real = kids[..first]
        .iter()
        .rposition(|e| e.as_node().is_some() || !cx.grammar.is_trivia(e.kind()))
        .map_or(0, |i| i + 1);
    let from = (real..first)
        .find(|&i| kids[i].kind() == k::NEWLINE)
        .map_or(real, |i| i + 1);
    let raw: String = kids[from..=last]
        .iter()
        .map(|e| match e {
            SyntaxElement::Token(t) => t.text().to_string(),
            SyntaxElement::Node(n) => n.text().to_string(),
        })
        .collect();
    parts.push(doc::line_suffix(doc::line_start(raw)));
    Ok(doc::concat(parts))
}

/// A compound command: head, opener, body, clauses, closer.
fn compound(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    let kind = node.kind();
    let arithmetic = kind == k::GROUP && !es.first().is_some_and(|e| e.is_kw(&["{"]));
    // A `for` list may continue over lines; elsewhere a continuation is kept by keeping the node.
    let continued = es
        .iter()
        .enumerate()
        .any(|(i, e)| e.cont && (kind != k::FOR || i + 1 == es.len()));
    if arithmetic || continued {
        return Ok(cx.verbatim_node(node));
    }
    let block = is_block(cx, node);
    let (mid, closer) = match es.split_last() {
        Some((c, mid)) if c.is_kw(&["fi", "done", "}", ")"]) => (mid, Some(c)),
        _ => (&es[..], None),
    };

    // `while`, `for`: the head, then the `do` group.
    if matches!(kind, k::WHILE | k::FOR | k::C_FOR) {
        let Some((body, head)) = mid.split_last() else {
            return Ok(cx.verbatim_node(node));
        };
        return Ok(doc::concat(vec![
            head_doc(cx, kind, head)?,
            doc::lit("; "),
            body.doc(cx)?,
        ]));
    }

    let oi = mid
        .iter()
        .position(|e| e.is_kw(&["then", "do", "else", "{", "("]));
    let Some(oi) = oi else {
        return Ok(cx.verbatim_node(node));
    };
    let (head, opener) = (&mid[..oi], &mid[oi]);
    let rest = &mid[oi + 1..];
    let split = rest
        .iter()
        .position(|e| e.is(k::ELIF) || e.is(k::ELSE))
        .unwrap_or(rest.len());
    let (body, clauses) = rest.split_at(split);

    let mut parts = Vec::new();
    if !head.is_empty() {
        parts.push(head_doc(cx, kind, head)?);
        parts.push(doc::lit("; "));
    }
    let open_tok = opener.tok().expect("an opener is a token");
    parts.push(if kind == k::ELSE {
        kw(cx, open_tok)
    } else {
        cx.token(open_tok)
    });
    let subshell = kind == k::SUBSHELL;
    if block {
        let after = clauses
            .first()
            .and_then(|c| c.first(cx))
            .or_else(|| closer.and_then(|c| c.tok().cloned()))
            .or_else(|| next_significant(cx, &mid_last(cx, mid)));
        let mut inner = Vec::new();
        if !body.is_empty() {
            inner.extend([
                doc::hardline(),
                opening_blank(cx, body),
                block_list(cx, body)?,
            ]);
        }
        inner.push(dangling(cx, after, !body.is_empty()));
        parts.push(doc::indent(doc::concat(inner)));
        for c in clauses {
            parts.push(doc::hardline());
            parts.push(c.doc(cx)?);
        }
        if let Some(c) = closer.and_then(E::tok) {
            parts.push(doc::hardline());
            parts.push(kw(cx, c));
        }
    } else {
        if !body.is_empty() {
            if !subshell {
                parts.push(doc::lit(" "));
            }
            parts.push(inline_list(cx, body, !subshell)?);
        }
        for c in clauses {
            parts.push(doc::lit(" "));
            parts.push(c.doc(cx)?);
        }
        if let Some(c) = closer.and_then(E::tok) {
            if !subshell {
                parts.push(doc::lit(" "));
            }
            parts.push(kw(cx, c));
        }
    }
    Ok(doc::concat(parts))
}

fn mid_last(cx: &FormatCtx<'_>, mid: &[E]) -> SyntaxToken {
    mid.iter()
        .rev()
        .find_map(|e| e.last(cx))
        .expect("a compound has tokens")
}

/// The blank lines the source has between the opener of a body and its first statement.
fn opening_blank(cx: &FormatCtx<'_>, body: &[E]) -> Doc {
    let first = body.iter().find_map(|e| e.first(cx));
    let blank = first.map_or(0, |t| comments::blank_lines_before(cx.grammar, &t));
    doc::concat(
        (0..blank.min(max_blank(cx)))
            .map(|_| doc::hardline())
            .collect(),
    )
}

/// Comments in front of the token that ends a body, inside the body (when they are deeper than
/// the token), or else a blank line.
fn dangling(cx: &FormatCtx<'_>, after: Option<SyntaxToken>, blank: bool) -> Doc {
    let Some(t) = after else { return Doc::Nil };
    let cs = comments::leading_comments(cx.grammar, &t);
    let cs = &cs[..deeper_comments(cx, &t)];
    if cs.is_empty() {
        // A blank line before the keyword stays.
        let blank_line = blank && comments::blank_lines_before(cx.grammar, &t) > 0;
        return if blank_line {
            doc::hardline()
        } else {
            Doc::Nil
        };
    }
    dangling_comments(
        cs,
        blank && comments::blank_lines_before(cx.grammar, &t) > 0,
    )
}

/// `if cond`, `elif cond`, `while cond`, `for i in words`, `for ((...))`.
fn head_doc(
    cx: &FormatCtx<'_>,
    kind: core_syntax::SyntaxKind,
    head: &[E],
) -> Result<Doc, FormatError> {
    let Some((first, rest)) = head.split_first() else {
        return Ok(Doc::Nil);
    };
    let Some(first_tok) = first.tok() else {
        return Ok(Doc::Nil);
    };
    let words: Vec<&E> = head.iter().filter(|e| !e.is(k::SEMI)).collect();
    if kind == k::C_FOR {
        let (a, b) = (first.first(cx), words.last().and_then(|e| e.last(cx)));
        return Ok(match (a, b) {
            (Some(a), Some(b)) => cx.verbatim_range(&a, &b),
            _ => Doc::Nil,
        });
    }
    let lead = if kind == k::ELIF {
        kw(cx, first_tok)
    } else {
        cx.token(first_tok)
    };
    if kind == k::FOR {
        let mut parts = vec![lead];
        let mut tail = Vec::new();
        for e in &words[1..] {
            let target = if tail.is_empty() && !e.cont {
                &mut parts
            } else {
                &mut tail
            };
            target.push(if e.cont {
                doc::lit(" \\")
            } else {
                doc::lit(" ")
            });
            if e.cont {
                target.push(doc::hardline());
            }
            target.push(e.doc(cx)?);
        }
        parts.push(doc::indent(doc::concat(tail)));
        return Ok(doc::concat(parts));
    }
    Ok(doc::concat(vec![
        lead,
        doc::lit(" "),
        inline_list(cx, rest, false)?,
    ]))
}

fn case(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    if es.iter().any(|e| e.cont) {
        return Ok(cx.verbatim_node(node));
    }
    let Some(ii) = es.iter().position(|e| e.is_kw(&["in"])) else {
        return Ok(cx.verbatim_node(node));
    };
    let (head, rest) = es.split_at(ii + 1);
    let (esac, items) = match rest.split_last() {
        Some((c, items)) if c.is_kw(&["esac"]) => (c, items),
        _ => return Ok(cx.verbatim_node(node)),
    };
    let g = cx.grammar;
    let mut parts = Vec::new();
    for (i, e) in head.iter().enumerate() {
        if i > 0 {
            parts.push(doc::lit(" "));
        }
        parts.push(e.doc(cx)?);
    }
    let block = is_block(cx, node);
    for e in items {
        if block {
            parts.push(doc::hardline());
            if let Some(first) = e.first(cx) {
                let blank = comments::blank_lines_before(g, &first).min(max_blank(cx));
                parts.extend((0..blank).map(|_| doc::hardline()));
            }
        } else {
            parts.push(doc::lit(" "));
        }
        parts.push(e.doc(cx)?);
    }
    parts.push(if block {
        doc::hardline()
    } else {
        doc::lit(" ")
    });
    if block
        && !items.is_empty()
        && esac
            .first(cx)
            .is_some_and(|t| comments::blank_lines_before(g, &t) > 0)
    {
        parts.push(doc::hardline());
    }
    parts.push(esac.doc(cx)?);
    Ok(doc::concat(parts))
}

/// `a | b)`, the body, `;;`. On one line when the source has it on one line.
fn case_item(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    let Some(pi) = es.iter().position(|e| e.is_kw(&[")"])) else {
        return Ok(cx.verbatim_node(node));
    };
    if es.iter().any(|e| e.cont) {
        return Ok(cx.verbatim_node(node));
    }
    if es[0].is_kw(&["("]) {
        return Ok(cx.verbatim_node(node));
    }
    let (pats, rest) = es.split_at(pi);
    let rest = &rest[1..];
    let (body, term) = match rest.split_last() {
        Some((t, body)) if t.op().is_some_and(|o| o.starts_with(";")) => (body, Some(t)),
        _ => (rest, None),
    };
    let mut parts = Vec::new();
    // Alternatives are separated by ` | `; the words of one pattern (`*"/.bashrc"`) touch.
    let mut after_bar = true;
    for e in pats {
        if e.op() == Some("|") {
            parts.push(doc::lit(" | "));
            after_bar = true;
            continue;
        }
        if !after_bar
            && e.first(cx).is_some_and(|t| {
                t.prev_token()
                    .is_some_and(|p| cx.grammar.is_trivia(p.kind()))
            })
        {
            parts.push(doc::lit(" "));
        }
        after_bar = false;
        parts.push(e.doc(cx)?);
    }
    parts.push(cx.token(es[pi].tok().expect("`)` is a token")));
    if is_block(cx, node) {
        let after = term.and_then(|t| t.tok().cloned());
        let mut inner = Vec::new();
        if !body.is_empty() {
            inner.extend([
                doc::hardline(),
                opening_blank(cx, body),
                block_list(cx, body)?,
            ]);
        }
        if term.is_some() {
            inner.push(dangling(cx, after, !body.is_empty()));
        }
        if let Some(t) = term.and_then(E::tok) {
            inner.extend([doc::hardline(), kw(cx, t)]);
        }
        parts.push(doc::indent(doc::concat(inner)));
    } else {
        if !body.is_empty() {
            parts.push(doc::lit(" "));
            parts.push(inline_list(cx, body, false)?);
        }
        if let Some(t) = term.and_then(E::tok) {
            parts.extend([doc::lit(" "), kw(cx, t)]);
        }
    }
    Ok(doc::concat(parts))
}

/// `name() body` and `function name body`.
fn function(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    if es.iter().any(|e| e.cont) {
        return Ok(cx.verbatim_node(node));
    }
    let mut parts = Vec::new();
    let mut glue = true;
    for e in &es {
        match e.kw() {
            Some("function") => {
                parts.push(e.doc(cx)?);
                parts.push(doc::lit(" "));
                glue = true;
            }
            Some(p @ ("(" | ")")) => {
                parts.push(doc::lit(if p == "(" { "(" } else { ")" }));
                glue = p == "(";
            }
            _ => {
                if !glue {
                    parts.push(doc::lit(" "));
                }
                parts.push(e.doc(cx)?);
                glue = false;
            }
        }
    }
    Ok(doc::concat(parts))
}
