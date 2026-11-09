//! INI-style files: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`),
//! formatting is the shared sequence shape plus the section layout in `format.rs`.

core_syntax::include_generated!();
mod format;
mod mapping;
mod rules;

pub use format::{IniFormatter, option_schema};

include!("provider.generated.rs");
