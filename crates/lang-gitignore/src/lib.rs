//! Gitignore-syntax files (`.gitignore`, `.dockerignore`, ...): tree-sitter parses (through
//! `core-treesitter`, mapping in `mapping.rs`), the formatter in `format.rs` trims and spaces lines.

mod format;
mod mapping;
mod rules;

pub use format::{GitignoreFormatter, option_schema};

include!("provider.generated.rs");
