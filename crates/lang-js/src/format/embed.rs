//! Code that is not a whole file: a host (HTML, Vue, Svelte) hands one expression, which the
//! provider parses as the only element of `[ ]` (`parse`) and prints here without the brackets.

use super::util::{R, shape_error};
use crate::generated::kind as k;
use core_format::{FormatCtx, doc::Doc};
use core_syntax::SyntaxNode;

/// The `embed_mode` option the host's engine sets (`EmbedMode::name`).
pub(crate) const EXPRESSION: &str = "expression";

pub(crate) fn format(
    node: &SyntaxNode,
    cx: &FormatCtx<'_>,
) -> Result<Option<Doc>, core_format::FormatError> {
    if node.kind() != k::PROGRAM {
        return Ok(None);
    }
    let embedded = matches!(cx.options.get("embed_mode"), Some(core_config::OptionValue::String(m)) if m == EXPRESSION);
    if !embedded {
        return Ok(None);
    }
    let mut items = node
        .descendants()
        .find(|n| n.kind() == k::ARRAY_EXPRESSION)
        .map(|a| a.children().collect::<Vec<_>>())
        .unwrap_or_default();
    let (Some(expr), true) = (items.pop(), items.is_empty()) else {
        return Err(shape_error("expression"));
    };
    let doc: R = cx.format(&expr);
    doc.map(Some)
}
