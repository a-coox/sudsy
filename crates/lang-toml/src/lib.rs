//! TOML: tree-sitter parses (through `core-treesitter`, mapping in `lang.yaml`), formatting is
//! the shared shapes plus the table layout in `format.rs`.

mod align;
mod format;
mod hooks;
mod rules;
mod validate;

pub use format::{ALIGN_COMMENTS, COLLAPSE, TRAILING_COMMAS, TomlFormatter, option_schema};

include!("provider.generated.rs");
