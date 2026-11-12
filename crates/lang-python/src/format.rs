//! Python formatting.

use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::SyntaxNode;

include!("schema.generated.rs");

pub struct PythonFormatter;

impl NodeFormatter for PythonFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok((node.parent().is_none()).then(|| cx.verbatim_node(node)))
    }
}
