//! HTML formatting: the document is printed as written.

use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, shapes};
use core_syntax::SyntaxNode;

include!("schema.generated.rs");

pub struct HtmlFormatter;

impl NodeFormatter for HtmlFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(shapes::verbatim(cx, node)))
    }
}
