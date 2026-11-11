//! CSS formatting: the text as written (the document is one verbatim piece).

use crate::generated::kind as k;
use core_config::shared;
use core_format::context::verbatim_text;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::SyntaxNode;

include!("schema.generated.rs");

pub struct CssFormatter;

impl NodeFormatter for CssFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        _cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok((node.kind() == k::STYLESHEET).then(|| verbatim_text(&node.text().to_string())))
    }
}
