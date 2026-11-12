//! HTML: tree-sitter parses (through `core-treesitter`, mapping in `lang.yaml`); the formatter
//! prints the file as written.

mod dialect;
mod format;
mod rules;

pub use dialect::{DialectProvider, vue_provider};
pub use format::{HtmlFormatter, option_schema};

include!("provider.generated.rs");
