//! YAML formatting. Block structure is printed with `indent` and `align`, flow collections with
//! the shared delimited-list shape, and comments through the shared comment placement; this module
//! holds the options and dispatches on the node kinds.
//!
//! * [`document`]: streams, documents and their markers. [`block`]: collections, entries and
//!   where a value goes after its indicator. [`scalars`]: scalar text, quote style and block
//!   scalar contents. [`comments`]: token printing and the comment placement hook for the end of
//!   block collections.

mod block;
mod comments;
mod document;
mod scalars;
pub(crate) use scalars::restyle;

use crate::generated::kind as k;
use crate::model;
use core_config::{ResolvedOptions, shared};
use core_format::comments::CommentStyle;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::SyntaxNode;

include!("../schema.generated.rs");

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceIndent {
    Preserve,
    Indented,
    Flush,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Quotes {
    Preserve,
    Single,
    Double,
    /// Double quotes, single when the text has a `"`.
    PreferDouble,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocumentEnd {
    Preserve,
    Always,
    Never,
}

pub(crate) enum DocumentStart {
    Preserve,
    Always,
    Never,
}

/// The options formatting code reads.
pub(crate) struct Style {
    pub indent: usize,
    pub sequence: SequenceIndent,
    pub quotes: Quotes,
    pub document_start: DocumentStart,
    pub document_end: DocumentEnd,
    pub max_blank: usize,
    /// Blank lines between `key:` and a block value on the next line are dropped.
    pub drop_blank_after_key: bool,
}

impl Style {
    fn of(options: &ResolvedOptions) -> Style {
        Style {
            drop_blank_after_key: options.str(BLANK_LINES_AFTER_KEY) == "remove",
            indent: options.int(shared::INDENT_SIZE) as usize,
            sequence: match options.str(SEQUENCE_INDENT) {
                "indented" => SequenceIndent::Indented,
                "flush" => SequenceIndent::Flush,
                _ => SequenceIndent::Preserve,
            },
            quotes: match options.str(QUOTE_STYLE) {
                "prefer_double" => Quotes::PreferDouble,
                "single" => Quotes::Single,
                "double" => Quotes::Double,
                _ => Quotes::Preserve,
            },
            document_start: match options.str(DOCUMENT_START) {
                "always" => DocumentStart::Always,
                "never" => DocumentStart::Never,
                _ => DocumentStart::Preserve,
            },
            document_end: match options.str(DOCUMENT_END) {
                "always" => DocumentEnd::Always,
                "never" => DocumentEnd::Never,
                _ => DocumentEnd::Preserve,
            },
            max_blank: options.int(shared::MAX_BLANK_LINES) as usize,
        }
    }
}

pub struct YamlFormatter;

impl NodeFormatter for YamlFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let st = cx.cached(|| Style::of(cx.options));
        let doc = match node.kind() {
            k::STREAM => document::stream(cx, st, node)?,
            k::BLOCK_MAPPING | k::BLOCK_SEQUENCE => block::collection(cx, st, node)?,
            k::BLOCK_MAPPING_ENTRY => block::mapping_entry(cx, st, node)?,
            k::BLOCK_SEQUENCE_ENTRY => block::sequence_entry(cx, st, node)?,
            k::FLOW_MAPPING | k::FLOW_SEQUENCE => block::flow_collection(cx, st, node)?,
            k::FLOW_PAIR => block::flow_pair(cx, node)?,
            k::PROPERTY_NODE => block::property_inline(cx, node)?,
            k::SCALAR => scalars::scalar(cx, st, node)?,
            k::ALIAS_NODE => scalars::alias(cx, node)?,
            k::BLOCK_SCALAR => scalars::block_scalar(cx, st, node, 0)?,
            _ => return Ok(None),
        };
        Ok(Some(doc))
    }

    fn comment_style(&self, options: &ResolvedOptions) -> CommentStyle {
        CommentStyle {
            gap: options.int(COMMENT_SPACING) as usize,
            ..CommentStyle::default()
        }
    }

    /// The data model must be the same before and after: indentation and quoting can change
    /// meaning, and the tree comparison cannot see it.
    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        let (a, b) = (model::load_tree(before)?, model::load_tree(after)?);
        if a.len() != b.len() {
            return Err(format!(
                "the data model changed: {} documents became {}",
                a.len(),
                b.len()
            ));
        }
        match a.iter().zip(&b).position(|(x, y)| x != y) {
            None => Ok(()),
            Some(i) => Err(format!("the data model of document {} changed", i + 1)),
        }
    }
}
