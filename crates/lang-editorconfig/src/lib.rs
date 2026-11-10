//! EditorConfig: tree-sitter parses (through `core-treesitter`, mapping in `lang.yaml`),
//! formatting is the shared sequence shape plus the section layout in `format.rs`.

mod format;
mod hooks;
mod rules;

pub use format::{EditorConfigFormatter, option_schema};

include!("provider.generated.rs");
