//! JSON formatting. Almost everything is the shared tag defaults; this file only maps the JSON
//! options onto the list shape for objects and arrays and handles the document's end.

use crate::generated::kind as k;
use core_config::{OptionDef, OptionSchema, OptionValue, Preset, ResolvedOptions, shared};
use core_format::shapes::{ListLayout, ListStyle, TrailingComma, delimited_list};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::SyntaxNode;

pub const COLLAPSE: &str = "collapse";
pub const TRAILING_COMMAS: &str = "trailing_commas";
pub const DIALECT: &str = "dialect";

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::line_width(80),
        shared::indent_size(2),
        shared::indent_style(),
        shared::final_newline(true),
        OptionDef::choice(
            COLLAPSE,
            &["preserve", "always", "never"],
            "preserve",
            "Objects and arrays on one line: `preserve` keeps one that has no line break after its \
             opening bracket on one line if it fits; `always` puts every one that fits on one \
             line; `never` puts every non-empty one on several lines.",
        ),
        OptionDef::choice(
            TRAILING_COMMAS,
            &["never", "multiline"],
            "never",
            "Trailing commas after the last item of a multi-line object or array (JSONC only).",
        ),
        OptionDef::choice(
            DIALECT,
            &["auto", "json", "jsonc"],
            "auto",
            "JSON or JSONC (comments, trailing commas); `auto` decides by file name.",
        ),
    ])
    .with_presets(presets())
}

pub struct JsonFormatter;

fn list_style(options: &ResolvedOptions, is_object: bool) -> ListStyle {
    let jsonc = options.str(DIALECT) == "jsonc";
    ListStyle {
        padding: is_object,
        trailing_comma: if jsonc && options.str(TRAILING_COMMAS) == "multiline" {
            TrailingComma::WhenBroken
        } else {
            TrailingComma::Never
        },
        layout: match options.str(COLLAPSE) {
            "always" => ListLayout::Fit,
            "never" => ListLayout::Break,
            _ if prettier(options) && is_object => ListLayout::PreserveOpenBreak,
            _ if prettier(options) => ListLayout::Fit,
            _ => ListLayout::PreserveBreak,
        },
        ..ListStyle::default()
    }
}

impl NodeFormatter for JsonFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let doc = match node.kind() {
            k::OBJECT => delimited_list(cx, node, &list_style(cx.options, true))?,
            k::ARRAY => delimited_list(cx, node, &list_style(cx.options, false))?,
            k::DOCUMENT => document(cx, node)?,
            _ => return Ok(None),
        };
        Ok(Some(doc))
    }
}

/// The value, then comments after it (which are owned by the end-of-file token).
fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    for child in node.children() {
        parts.push(cx.format(&child)?);
    }
    let eof = cx
        .last_token(node)
        .or_else(|| node.last_token())
        .ok_or_else(|| FormatError::Shape("Document without tokens".into()))?;
    let trailing = comments::leading_comments(cx.grammar, &eof);
    if !trailing.is_empty() {
        if !parts.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(core_format::context::leading_comments_doc(&trailing));
    }
    Ok(doc::concat(parts))
}

/// `json/default` is the schema's defaults (two spaces, width 80). `json/prettier-compat` pins
/// the values that reproduce Prettier's JSON output.
fn presets() -> Vec<Preset> {
    vec![
        Preset::new("json/default@1", vec![]),
        Preset::new(
            "json/prettier-compat@1",
            vec![
                (shared::LINE_WIDTH, OptionValue::Int(80)),
                (shared::INDENT_SIZE, OptionValue::Int(2)),
                (COLLAPSE, OptionValue::String("preserve".into())),
                (TRAILING_COMMAS, OptionValue::String("never".into())),
                (LAYOUT, OptionValue::String("prettier".into())),
            ],
        ),
    ]
}

/// Preset-internal value (not in the schema, so users cannot set it): `prettier` makes `preserve`
/// follow Prettier: an object stays expanded when its first key is on a line of its own, an array
/// is on one line whenever it fits.
const LAYOUT: &str = "layout";

fn prettier(options: &ResolvedOptions) -> bool {
    matches!(options.get(LAYOUT), Some(OptionValue::String(s)) if s == "prettier")
}
