//! HTML: tree-sitter parses (through `core-treesitter`, mapping in `lang.yaml`); the formatter
//! prints the file as written.

mod checks;
mod document;
mod dom;
mod format;
mod hooks;
mod meta;
mod rules;

pub use format::{HtmlFormatter, option_schema};

include!("provider.generated.rs");
