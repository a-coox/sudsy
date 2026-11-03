//! The context formatting code runs in, and the trait providers implement.

use crate::comments::{self, Comment, CommentKind, CommentStyle};
use crate::directives::{Directive, DirectiveTable, NodeDirectives};
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::ignore::{self, IgnoreMark};
use crate::lines::LineBreaks;
use crate::shapes::{self, BlockStyle, KeyValueStyle, ListStyle};
use core_config::ResolvedOptions;
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, SyntaxToken, TextRange};
use std::sync::Mutex;

/// A provider's formatting code. Nodes it does not handle fall back to the shared shape of their
/// grammar tag (`#block`, `#delimited-list`, `#key-value`), and else to their children
/// concatenated.
pub trait NodeFormatter: Send + Sync {
    /// Layout for `node`, or `None` to use the tag default. Per-node code builds a style from
    /// `cx.options` and calls a shape; hooks that try several layouts use `conditional_group`.
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError>;

    /// A provider-specific addition to the generic safety check (YAML compares the loaded data
    /// model here). `before` and `after` are the trees of the source and of the formatted
    /// output, both already parsed without errors.
    fn extra_check(&self, _before: &SyntaxNode, _after: &SyntaxNode) -> Result<(), String> {
        Ok(())
    }

    /// How comments are printed; the default keeps their text and puts one space before a
    /// comment after code.
    fn comment_style(&self, _options: &ResolvedOptions) -> CommentStyle {
        CommentStyle::default()
    }
}

/// For providers that have no formatting code yet: everything uses tag defaults.
pub struct DefaultFormatter;

impl NodeFormatter for DefaultFormatter {
    fn format_node(&self, _: &SyntaxNode, _: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
        Ok(None)
    }
}

/// Everything a formatting decision may read. Any node of a tree can be formatted with
/// [`FormatCtx::format`]: nothing depends on the root, so a subtree is formatted in its context
/// (its comments and neighbours are read from the tree around it).
pub struct FormatCtx<'a> {
    pub grammar: &'a Grammar,
    pub options: &'a ResolvedOptions,
    pub directives: &'a DirectiveTable,
    formatter: &'a dyn NodeFormatter,
    pub comment_style: CommentStyle,
    /// The line breaks of the tree being formatted, found when first asked for.
    line_breaks: Mutex<Option<LineBreaks>>,
}

impl<'a> FormatCtx<'a> {
    pub fn new(
        grammar: &'a Grammar,
        options: &'a ResolvedOptions,
        directives: &'a DirectiveTable,
        formatter: &'a dyn NodeFormatter,
    ) -> Self {
        FormatCtx {
            grammar,
            options,
            directives,
            formatter,
            comment_style: formatter.comment_style(options),
            line_breaks: Mutex::new(None),
        }
    }

    /// Does the source contain a line break (`\n` or `\r`) within `range`, which is given in the
    /// offsets of the tree `node` is in?
    pub fn has_line_break(&self, node: &SyntaxNode, range: TextRange) -> bool {
        let root = node.ancestors().last().unwrap_or_else(|| node.clone());
        let mut cached = self.line_breaks.lock().unwrap_or_else(|e| e.into_inner());
        if !cached.as_ref().is_some_and(|l| l.is_of(&root)) {
            *cached = Some(LineBreaks::of(&root));
        }
        cached.as_ref().is_some_and(|l| l.any_in(range))
    }

    pub fn directives_of(&self, node: &SyntaxNode) -> NodeDirectives<'a> {
        self.directives.of(node)
    }

    /// Formats a child node. Honors `sudsy-ignore` comments before the node and the `Preserve`
    /// directive; use [`FormatCtx::format_unguarded`] for the node a caller was invoked for.
    pub fn format(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        // Only a comment can ask for the node to be kept as written.
        if comments::node_leading(self.grammar, node).has_comment
            && let Some(first) = self.first_token(node)
        {
            let leading = comments::leading_comments(self.grammar, &first);
            if ignore::mark_in(self.grammar, &leading) == Some(IgnoreMark::Ignore) {
                return Ok(self.verbatim_node(node));
            }
        }
        if self.directives_of(node).has(&Directive::Preserve) {
            return Ok(self.verbatim_node(node));
        }
        self.format_unguarded(node)
    }

    /// Formatting code for `node` itself, ignoring ignore comments and `Preserve`.
    pub fn format_unguarded(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        match self.formatter.format_node(node, self)? {
            Some(doc) => Ok(doc),
            None => self.format_default(node),
        }
    }

    /// The shared shape for the node's grammar tag.
    pub fn format_default(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        let g = self.grammar;
        let has = |tag| g.def(node.kind()).is_some_and(|d| d.has_tag(tag));
        if has("delimited-list") {
            shapes::delimited_list(self, node, &ListStyle::default())
        } else if has("block") {
            shapes::block(self, node, &BlockStyle::default())
        } else if has("key-value") {
            shapes::key_value(self, node, &KeyValueStyle::default())
        } else {
            self.concat_children(node)
        }
    }

    /// Children printed one after the other, with no spacing.
    pub fn concat_children(&self, node: &SyntaxNode) -> Result<Doc, FormatError> {
        // The first part waits in `single`: most nodes have just one, and need no list.
        let mut single = None;
        let mut parts = Vec::new();
        let significant = node
            .children_with_tokens()
            .by_kind(|k| !self.grammar.is_trivia(k));
        for e in significant {
            let part = match e {
                SyntaxElement::Node(n) => self.format(&n)?,
                SyntaxElement::Token(t) => self.token(&t),
            };
            match single.take() {
                None if parts.is_empty() => single = Some(part),
                None => parts.push(part),
                Some(first) => parts.extend([first, part]),
            }
        }
        Ok(single.unwrap_or_else(|| doc::concat(parts)))
    }

    pub fn first_token(&self, node: &SyntaxNode) -> Option<SyntaxToken> {
        let mut walk = node.preorder_with_tokens();
        while let Some(event) = walk.next() {
            match event {
                rowan::WalkEvent::Enter(SyntaxElement::Node(n))
                    if self.grammar.is_trivia(n.kind()) =>
                {
                    walk.skip_subtree();
                }
                rowan::WalkEvent::Enter(SyntaxElement::Token(t))
                    if !self.grammar.is_trivia(t.kind()) =>
                {
                    return Some(t);
                }
                _ => {}
            }
        }
        None
    }

    pub fn last_token(&self, node: &SyntaxNode) -> Option<SyntaxToken> {
        // Walk back from the node's last token over trivia, staying inside the node.
        let mut cur = node.last_token();
        while let Some(t) = cur {
            if !self.grammar.is_trivia(t.kind()) && !self.in_trivia_node(&t, node) {
                return Some(t);
            }
            cur = t
                .prev_token()
                .filter(|p| p.parent_ancestors().any(|a| a == *node));
        }
        None
    }

    /// Is `t` inside a structured trivia node (a doc comment) below `node`?
    fn in_trivia_node(&self, t: &SyntaxToken, node: &SyntaxNode) -> bool {
        t.parent_ancestors()
            .take_while(|a| a != node)
            .any(|a| self.grammar.is_trivia(a.kind()))
    }

    /// A token with its leading and trailing comments.
    pub fn token(&self, tok: &SyntaxToken) -> Doc {
        let leading = comments::leading_comments(self.grammar, tok);
        let trailing = comments::trailing_comments(self.grammar, tok);
        if leading.is_empty() && trailing.is_empty() {
            return self.token_text(tok);
        }
        doc::concat(vec![
            leading_comments_doc(&self.comment_style, &leading),
            self.token_text(tok),
            trailing_comments_doc(&self.comment_style, &trailing),
        ])
    }

    pub fn token_text(&self, tok: &SyntaxToken) -> Doc {
        verbatim_text(tok.text())
    }

    pub fn leading_comments_doc(&self, tok: &SyntaxToken) -> Doc {
        leading_comments_doc(
            &self.comment_style,
            &comments::leading_comments(self.grammar, tok),
        )
    }

    pub fn trailing_comments_doc(&self, tok: &SyntaxToken) -> Doc {
        trailing_comments_doc(
            &self.comment_style,
            &comments::trailing_comments(self.grammar, tok),
        )
    }

    /// Source text from the node's first to its last token, plus those tokens' outer comments.
    pub fn verbatim_node(&self, node: &SyntaxNode) -> Doc {
        match (self.first_token(node), self.last_token(node)) {
            (Some(first), Some(last)) => self.verbatim_range(&first, &last),
            _ => Doc::Nil,
        }
    }

    /// Source text from the start of `first` to the end of `last`, exactly as written (including
    /// the comments between them), with `first`'s leading and `last`'s trailing comments around it.
    pub fn verbatim_range(&self, first: &SyntaxToken, last: &SyntaxToken) -> Doc {
        let root = first.parent().map(|p| p.ancestors().last().unwrap_or(p));
        let raw = root
            .map(|r| {
                let start = first.text_range().start() - r.text_range().start();
                let end = last.text_range().end() - r.text_range().start();
                r.text()
                    .slice(core_syntax::TextRange::new(start, end))
                    .to_string()
            })
            .unwrap_or_default();
        doc::concat(vec![
            self.leading_comments_doc(first),
            verbatim_text(&raw),
            self.trailing_comments_doc(last),
        ])
    }
}

/// Text that may span several lines: a multi-line text breaks the groups around it.
pub fn verbatim_text(s: &str) -> Doc {
    if s.contains('\n') || s.contains('\r') {
        doc::concat(vec![doc::text(s), doc::break_parent()])
    } else {
        doc::snippet(s)
    }
}

/// One own-line comment (or kept-as-written text) as a document, without the break after it.
pub fn comment_doc(st: &CommentStyle, c: &Comment) -> Doc {
    let text = c.styled_text(st);
    match c.kind {
        CommentKind::Lines => lines_doc(&text),
        CommentKind::Directive => doc::concat(vec![doc::line_start(text), doc::break_parent()]),
        CommentKind::Line | CommentKind::Block => verbatim_text(&text),
    }
}

/// The lines of `text` with their indentation replaced by the current one.
fn lines_doc(text: &str) -> Doc {
    let mut parts = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            parts.push(doc::hardline());
        }
        parts.push(doc::snippet(line.trim_start()));
    }
    doc::concat(parts)
}

/// Own-line comments go on their own lines, inline block comments stay inline.
pub fn leading_comments_doc(st: &CommentStyle, cs: &[Comment]) -> Doc {
    if cs.is_empty() {
        return Doc::Nil;
    }
    let mut parts = Vec::new();
    for c in cs {
        // A comment that was on its own line starts one, wherever the formatter put the code
        // before it.
        if c.kind != CommentKind::Block || c.breaks_after > 0 {
            parts.push(doc::newline_if_needed());
        }
        parts.push(comment_doc(st, c));
        if c.kind == CommentKind::Line || c.breaks_after > 0 {
            parts.push(doc::hardline());
            if c.breaks_after >= 2 {
                parts.push(doc::hardline());
            }
        } else {
            parts.push(doc::lit(" "));
        }
    }
    doc::concat(parts)
}

/// Comments after a token on its line: line comments are deferred to the end of the line and
/// force the enclosing group to break.
pub fn trailing_comments_doc(st: &CommentStyle, cs: &[Comment]) -> Doc {
    if cs.is_empty() {
        return Doc::Nil;
    }
    let mut parts = Vec::new();
    for c in cs {
        let body = doc::concat(vec![st.gap_doc(), verbatim_text(&c.styled_text(st))]);
        match c.kind {
            CommentKind::Line | CommentKind::Lines | CommentKind::Directive => {
                parts.push(doc::line_suffix(body));
                parts.push(doc::break_parent());
            }
            CommentKind::Block => parts.push(body),
        }
    }
    doc::concat(parts)
}
