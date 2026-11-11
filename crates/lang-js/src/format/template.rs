//! Template literals keep their text as written; a tagged template is its tag, type arguments and
//! template.

use crate::generated::kind as k;
use core_format::doc::Doc;
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    match node.kind() {
        k::TAGGED_TEMPLATE_EXPRESSION => Ok(Some(cx.concat_children(node)?)),
        _ => Ok(None),
    }
}
