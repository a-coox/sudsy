//! JavaScript and TypeScript formatting: the file is printed as written, with comments and blank
//! lines at its edges normalized like in every language.

use crate::generated::kind as k;
use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::SyntaxNode;

include!("schema.generated.rs");

pub struct JsFormatter;

impl NodeFormatter for JsFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok((node.kind() == k::PROGRAM).then(|| cx.verbatim_node(node)))
    }
}
