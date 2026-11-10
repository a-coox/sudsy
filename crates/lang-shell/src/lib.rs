//! POSIX sh and Bash: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`),
//! the formatter in `format.rs` lays out the statements and keeps every word as written.

mod format;
mod mapping;
mod rules;

pub use format::{ShellFormatter, option_schema};

include!("provider.generated.rs");
