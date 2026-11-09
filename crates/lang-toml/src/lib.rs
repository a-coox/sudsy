//! TOML: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`), formatting is
//! the shared shapes plus the table layout in `format.rs`.

core_syntax::include_generated!();
mod align;
mod format;
mod hooks;
mod mapping;
mod rules;
mod validate;

pub use format::{
    ALIGN_COMMENTS, COLLAPSE, MAX_BLANK_LINES, TRAILING_COMMAS, TomlFormatter, option_schema,
};

include!("provider.generated.rs");
