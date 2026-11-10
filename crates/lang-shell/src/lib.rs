//! POSIX sh and Bash: tree-sitter parses (through `core-treesitter`, mapping in `lang.yaml`),
//! the formatter in `format.rs` lays out the statements and keeps every word as written.

mod format;
mod rules;

pub use format::{ShellFormatter, option_schema};

include!("provider.generated.rs");
