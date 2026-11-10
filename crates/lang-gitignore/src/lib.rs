//! Gitignore-syntax files (`.gitignore`, `.dockerignore`, ...): tree-sitter parses (through
//! `core-treesitter`, mapping in `lang.yaml`), the formatter in `format.rs` trims and spaces lines.

mod format;
mod rules;

pub use format::{GitignoreFormatter, option_schema};

include!("provider.generated.rs");
