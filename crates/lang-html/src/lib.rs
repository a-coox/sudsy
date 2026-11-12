//! HTML: tree-sitter parses (through `core-treesitter`, mapping in `lang.yaml`); the formatter
//! prints the file as written.

mod format;
mod rules;

pub use format::{HtmlFormatter, option_schema};

include!("provider.generated.rs");
