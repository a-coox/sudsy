//! The context formatting code runs in, and the trait providers implement.

use crate::comments::{self, Comment, CommentKind, CommentStyle};
use crate::directives::{Directive, DirectiveTable, NodeDirectives};
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::ignore::{self, IgnoreMark};
use crate::lines::LineBreaks;
use crate::shapes::{self, BlockStyle, KeyValueStyle, ListStyle};
use core_config::ResolvedOptions;
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, SyntaxToken, TextRange, TextSize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Mutex;

/// What an embedded region holds (see [`Embedder`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbedMode {
    /// A whole file of the language.
    Program,
    /// One expression (a Vue directive, `{{ }}`, a Svelte `{...}`).
    Expression,
    /// A list of declarations (`style=""`).
    Declarations,
}

impl EmbedMode {
    /// The name the other language's provider reads in its `embed_mode` option.
    pub fn name(self) -> &'static str {
        match self {
            EmbedMode::Program => "program",
            EmbedMode::Expression => "expression",
            EmbedMode::Declarations => "declarations",
        }
    }
}

/// Formats code of another language for a host formatter (`<script>` in HTML): the engine
/// parses `code` with the provider of `language`, formats it for `width` columns and returns the
/// text without a final line break, or `None` when the code does not parse or the language is
/// unknown. `single_quotes`: the host's own quote is `"` (an attribute value), so strings prefer `'`.
pub trait Embedder {
    fn format(
        &self,
        language: &str,
        mode: EmbedMode,
        code: &str,
        width: usize,
        single_quotes: bool,
    ) -> Option<String>;
}

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

    /// What the safety check compares of a `#restyle` token or node of the input and of the
    /// output: the text without what formatting may change (quotes, case, number spelling, blanks).
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) => Cow::Borrowed(t.text()),
            SyntaxElement::Node(n) => Cow::Owned(n.text().to_string()),
        }
    }

    /// Source text to format instead of the input, for an option that reorders code (C# sorts
    /// `using` directives): the input tree is parsed from it again, and the safety check compares
    /// the output with that tree. `None` (the default) formats the input as it is.
    fn rewrite_source(
        &self,
        _tree: &SyntaxNode,
        _text: &str,
        _grammar: &Grammar,
        _options: &ResolvedOptions,
    ) -> Option<String> {
        None
    }

    /// Formatted text of the code the input's own parse leaves out (C# `#if` branches that are
    /// inactive under the file's symbols): `output` is the input formatted from `tree`;
    /// `variants` parses and formats the same text under other symbol sets. Returns the output
    /// with those parts replaced by their formatted text, and the symbol sets whose parses it
    /// took text from. The caller checks the result against the tree of every one of them and
    /// falls back to `output` on any doubt. `None` (the default) means there is nothing to merge.
    fn merge_variants(
        &self,
        _tree: &SyntaxNode,
        _output: &str,
        _options: &ResolvedOptions,
        _variants: &mut dyn Variants,
    ) -> Option<(String, Vec<Vec<String>>)> {
        None
    }

    /// How comments are printed; the default keeps their text and puts one space before a
    /// comment after code.
    fn comment_style(&self, _options: &ResolvedOptions) -> CommentStyle {
        CommentStyle::default()
    }
}

/// Other parses of the text being formatted, under other symbol sets (see
/// [`NodeFormatter::merge_variants`]). Parses are memoized by symbol set.
pub trait Variants {
    /// The tree under `symbols`, or `None` when it has syntax errors.
    fn parse(&mut self, symbols: &[String]) -> Option<SyntaxNode>;
    /// `tree` (from [`Variants::parse`]) formatted; `None` when formatting fails.
    fn format(&mut self, tree: &SyntaxNode) -> Option<String>;
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
    /// Tokens whose trailing comments a shape prints itself, with the comments once the token
    /// was printed.
    detached: Mutex<Vec<(TextSize, Option<Vec<Comment>>)>>,
    /// Like `detached` for the whole run: the comments are read, not taken, so a document that
    /// was cached while they were held back stays right when it is used again.
    held: Mutex<Vec<(TextSize, Option<Vec<Comment>>)>>,
    /// A provider's per-run data derived from the options (see [`FormatCtx::cached`]).
    cache: std::sync::OnceLock<Box<dyn std::any::Any + Send + Sync>>,
    /// Documents of nodes formatted for a layout, by node and flags (see [`FormatCtx::layout`]).
    layouts: Mutex<HashMap<(TextRange, u32, u32), Doc>>,
    embedder: Option<&'a dyn Embedder>,
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
            detached: Mutex::new(Vec::new()),
            held: Mutex::new(Vec::new()),
            cache: std::sync::OnceLock::new(),
            layouts: Mutex::new(HashMap::new()),
            embedder: None,
        }
    }

    /// The engine's formatter of other languages' code, when the host runs under one.
    pub fn with_embedder(mut self, embedder: Option<&'a dyn Embedder>) -> Self {
        self.embedder = embedder;
        self
    }

    /// `code` of `language` formatted for `width` columns (see [`Embedder`]).
    pub fn embed(
        &self,
        language: &str,
        mode: EmbedMode,
        code: &str,
        width: usize,
        single_quotes: bool,
    ) -> Option<String> {
        self.embedder?
            .format(language, mode, code, width, single_quotes)
    }

    /// Data the provider derives from the options once per run instead of once per node. A
    /// context serves one provider, so `T` is always the same type.
    pub fn cached<T: std::any::Any + Send + Sync>(&self, make: impl FnOnce() -> T) -> &T {
        self.cache
            .get_or_init(|| Box::new(make()))
            .downcast_ref::<T>()
            .expect("one cached type per FormatCtx")
    }

    /// The document `make` builds for `node` under `flags` (a provider's own numbering of the
    /// layouts it tries), built once: a layout that prints a child in two candidates (the last
    /// call argument hugged or not) reuses the child's document instead of formatting it again,
    /// which would take 2^depth runs for nested callbacks. The result is [`doc::shared`], so
    /// clones cost nothing.
    pub fn layout(
        &self,
        node: &SyntaxNode,
        flags: u32,
        make: impl FnOnce() -> Result<Doc, FormatError>,
    ) -> Result<Doc, FormatError> {
        let key = (node.text_range(), node.kind().0, flags);
        if let Some(d) = self
            .layouts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
        {
            return Ok(d.clone());
        }
        let d = doc::shared(make()?);
        self.layouts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, d.clone());
        Ok(d)
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
        crate::deep(|| match self.formatter.format_node(node, self)? {
            Some(doc) => Ok(doc),
            None => self.format_default(node),
        })
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
        let leading = self.leading(tok);
        let trailing = self.trailing(tok);
        if leading.is_empty() && trailing.is_empty() {
            return self.token_text(tok);
        }
        // Inline comments in front of a closing token (tag `#close`) sit tight against it.
        let lead = if self.grammar.has_tag(tok.kind(), "close")
            && leading
                .iter()
                .all(|c| c.kind == CommentKind::Block && c.breaks_after == 0)
        {
            doc::concat(leading.iter().map(comment_doc).collect())
        } else {
            leading_comments_doc(&leading)
        };
        doc::concat(vec![
            lead,
            self.token_text(tok),
            trailing_comments_doc(&self.comment_style, &trailing),
        ])
    }

    pub fn token_text(&self, tok: &SyntaxToken) -> Doc {
        verbatim_text(tok.text())
    }

    pub fn leading_comments_doc(&self, tok: &SyntaxToken) -> Doc {
        leading_comments_doc(&self.leading(tok))
    }

    /// The comments in front of `tok`, as the comment style wants them attached to it.
    fn leading(&self, tok: &SyntaxToken) -> Vec<Comment> {
        let mut cs = comments::leading_comments(self.grammar, tok);
        if self.comment_style.attach_comments
            && let Some(last) = cs.last_mut()
            && matches!(last.kind, CommentKind::Line | CommentKind::Lines)
            && last.breaks_after >= 2
            && !last.text().starts_with("////")
            && !(last.kind == CommentKind::Lines && last.text().starts_with('#'))
            && !comments::starts_file(self.grammar, tok)
        {
            last.breaks_after = 1;
        }
        cs
    }

    pub fn trailing_comments_doc(&self, tok: &SyntaxToken) -> Doc {
        trailing_comments_doc(&self.comment_style, &self.trailing(tok))
    }

    /// The trailing comments of `tok`, none while a shape has detached them.
    fn trailing(&self, tok: &SyntaxToken) -> Vec<Comment> {
        let cs = comments::trailing_comments(self.grammar, tok);
        if !cs.is_empty() {
            let at = tok.text_range().start();
            let mut held = self.held.lock().expect("held lock");
            if let Some(h) = held.iter_mut().find(|h| h.0 == at) {
                h.1 = Some(cs);
                return Vec::new();
            }
            drop(held);
            let mut detached = self.detached.lock().expect("detached lock");
            if let Some(d) = detached.iter_mut().find(|d| d.0 == at) {
                d.1 = Some(cs);
                return Vec::new();
            }
        }
        cs
    }

    /// Holds back the trailing comments of `tok` from the next printing of the token, so that a
    /// shape can print them itself after what follows the token (a list separator).
    pub fn detach_trailing(&self, tok: &SyntaxToken) {
        let mut detached = self.detached.lock().expect("detached lock");
        detached.push((tok.text_range().start(), None));
    }

    /// [`FormatCtx::detach_trailing`] for the rest of the run, ended by nothing: read the comments
    /// with [`FormatCtx::held_trailing`] as often as needed.
    pub fn hold_trailing(&self, tok: &SyntaxToken) {
        let at = tok.text_range().start();
        let mut held = self.held.lock().expect("held lock");
        if !held.iter().any(|h| h.0 == at) {
            held.push((at, None));
        }
    }

    /// The comments [`FormatCtx::hold_trailing`] held back, as the printed trailing comments of the
    /// token; nothing when the token was not printed through the context.
    pub fn held_trailing(&self, tok: &SyntaxToken) -> Doc {
        let at = tok.text_range().start();
        let held = self.held.lock().expect("held lock");
        held.iter()
            .find(|h| h.0 == at)
            .and_then(|h| h.1.as_ref())
            .map_or(Doc::Nil, |cs| {
                trailing_comments_doc(&self.comment_style, cs)
            })
    }

    /// Ends [`FormatCtx::detach_trailing`]: the comments of a token that was printed without them
    /// (the caller prints them), none when the token was not printed through the context (its
    /// comments are part of what was printed).
    pub fn reattach_trailing(&self, tok: &SyntaxToken) -> Doc {
        let at = tok.text_range().start();
        let mut detached = self.detached.lock().expect("detached lock");
        let i = detached.iter().position(|d| d.0 == at);
        i.and_then(|i| detached.swap_remove(i).1)
            .map_or(Doc::Nil, |cs| {
                trailing_comments_doc(&self.comment_style, &cs)
            })
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
        // Sliced from the nearest node that holds both tokens: slicing the root walks the file.
        let span = core_syntax::TextRange::new(first.text_range().start(), last.text_range().end());
        let raw = first
            .parent_ancestors()
            .find(|a| a.text_range().contains_range(span))
            .map(|n| {
                let at = n.text_range().start();
                n.text()
                    .slice(core_syntax::TextRange::new(
                        span.start() - at,
                        span.end() - at,
                    ))
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
pub fn comment_doc(c: &Comment) -> Doc {
    let text = c.text();
    match c.kind {
        CommentKind::Lines => lines_doc(&text),
        CommentKind::Directive => doc::concat(vec![doc::line_start(text), doc::break_parent()]),
        CommentKind::Line | CommentKind::Block => verbatim_text(&text),
    }
}

/// The lines of `text` with their indentation replaced by the current one.
fn lines_doc(text: &str) -> Doc {
    let mut parts = Vec::with_capacity(2 * text.lines().count());
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_start();
        if i > 0 {
            parts.push(doc::hardline());
            if line.starts_with('*') {
                parts.push(doc::text(format!(" {line}")));
                continue;
            }
        }
        parts.push(doc::snippet(line));
    }
    doc::concat(parts)
}

/// Own-line comments go on their own lines, inline block comments stay inline.
pub fn leading_comments_doc(cs: &[Comment]) -> Doc {
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
        parts.push(comment_doc(c));
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
        let body = doc::concat(vec![st.gap_before(c), verbatim_text(&c.text())]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use core_syntax::{TreeBuilder, grammars};
    use std::cell::Cell;

    #[test]
    fn layout_builds_once_per_node_and_flags() {
        let g = grammars::json();
        let mut b = TreeBuilder::new();
        b.start_node(g.kind_by_name("Document").unwrap());
        b.token(g.kind_by_name("Number").unwrap(), "1");
        b.finish_node();
        let root = SyntaxNode::new_root(b.finish());
        let (options, table) = (ResolvedOptions::default(), DirectiveTable::default());
        let cx = FormatCtx::new(g, &options, &table, &DefaultFormatter);
        let runs = Cell::new(0);
        let make = || {
            runs.set(runs.get() + 1);
            Ok(doc::concat(vec![doc::lit("a"), doc::line()]))
        };
        let first = cx.layout(&root, 0, make).unwrap();
        assert_eq!(cx.layout(&root, 0, make).unwrap(), first);
        assert_eq!(runs.get(), 1);
        cx.layout(&root, 1, make).unwrap();
        assert_eq!(runs.get(), 2);
    }
}
