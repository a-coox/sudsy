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

use crate::generated::kind as k;
use crate::model;
use core_config::{OptionDef, OptionSchema, OptionValue, Preset, ResolvedOptions, shared};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::SyntaxNode;

pub const SEQUENCE_INDENT: &str = "sequence_indent";
pub const QUOTE_STYLE: &str = "quote_style";
pub const DOCUMENT_START: &str = "document_start";
pub const MAX_BLANK_LINES: &str = "max_blank_lines";

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::line_width(80),
        shared::indent_size(2),
        shared::final_newline(true),
        OptionDef::choice(
            SEQUENCE_INDENT,
            &["preserve", "indented", "flush"],
            "preserve",
            "A block sequence that is the value of a mapping key: `indented` puts the dashes one \
             level in, `flush` at the key's column, `preserve` keeps each sequence as written.",
        ),
        OptionDef::choice(
            QUOTE_STYLE,
            &["preserve", "single", "double"],
            "preserve",
            "Quote style of quoted scalars. A scalar keeps its quotes when the other style would \
             change its value or need more escapes.",
        ),
        OptionDef::choice(
            DOCUMENT_START,
            &["preserve", "always", "never"],
            "preserve",
            "The `---` marker: `always` starts every document with it, `never` leaves it out of \
             the first document when that is possible, `preserve` keeps what is written.",
        ),
        OptionDef::int(
            MAX_BLANK_LINES,
            1,
            (0, 10),
            "Largest number of blank lines kept between entries, comments and documents.",
        ),
    ])
    .with_presets(presets())
}

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
    /// Prettier's rule (`yaml/prettier-compat`): double quotes, single when the text has a `"`.
    Prettier,
}

#[derive(Clone, Copy, PartialEq, Eq)]
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
    pub max_blank: usize,
    /// Blank lines between `key:` and a block value on the next line are dropped.
    pub tight_values: bool,
}

impl Style {
    fn of(options: &ResolvedOptions) -> Style {
        let prettier = prettier(options);
        Style {
            tight_values: prettier,
            indent: options.int(shared::INDENT_SIZE) as usize,
            sequence: match options.str(SEQUENCE_INDENT) {
                "indented" => SequenceIndent::Indented,
                "flush" => SequenceIndent::Flush,
                _ => SequenceIndent::Preserve,
            },
            quotes: match options.str(QUOTE_STYLE) {
                _ if prettier => Quotes::Prettier,
                "single" => Quotes::Single,
                "double" => Quotes::Double,
                _ => Quotes::Preserve,
            },
            document_start: match options.str(DOCUMENT_START) {
                "always" => DocumentStart::Always,
                "never" => DocumentStart::Never,
                _ => DocumentStart::Preserve,
            },
            max_blank: options.int(MAX_BLANK_LINES) as usize,
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
        let st = Style::of(cx.options);
        let doc = match node.kind() {
            k::STREAM => document::stream(cx, &st, node)?,
            k::BLOCK_MAPPING | k::BLOCK_SEQUENCE => block::collection(cx, &st, node)?,
            k::BLOCK_MAPPING_ENTRY => block::mapping_entry(cx, &st, node)?,
            k::BLOCK_SEQUENCE_ENTRY => block::sequence_entry(cx, &st, node)?,
            k::FLOW_MAPPING | k::FLOW_SEQUENCE => block::flow_collection(cx, &st, node)?,
            k::FLOW_PAIR => block::flow_pair(cx, node)?,
            k::PROPERTY_NODE => block::property_inline(cx, node)?,
            k::SCALAR => scalars::scalar(cx, &st, node)?,
            k::ALIAS_NODE => scalars::alias(cx, node)?,
            k::BLOCK_SCALAR => scalars::block_scalar(cx, &st, node, 0)?,
            _ => return Ok(None),
        };
        Ok(Some(doc))
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

/// `yaml/default` is the schema's defaults: conservative, every style kept as written.
/// `yaml/prettier-compat` pins the values that reproduce Prettier's YAML output.
fn presets() -> Vec<Preset> {
    vec![
        Preset::new("yaml/default@1", vec![]),
        Preset::new(
            "yaml/prettier-compat@1",
            vec![
                (shared::LINE_WIDTH, OptionValue::Int(80)),
                (shared::INDENT_SIZE, OptionValue::Int(2)),
                (SEQUENCE_INDENT, OptionValue::String("indented".into())),
                (DOCUMENT_START, OptionValue::String("preserve".into())),
                (MAX_BLANK_LINES, OptionValue::Int(1)),
                (LAYOUT, OptionValue::String("prettier".into())),
            ],
        ),
    ]
}

/// Preset-internal value (not in the schema, so users cannot set it): `prettier` selects the
/// layout rules that reproduce Prettier's output beyond the public options.
const LAYOUT: &str = "layout";

fn prettier(options: &ResolvedOptions) -> bool {
    matches!(options.get(LAYOUT), Some(OptionValue::String(s)) if s == "prettier")
}
