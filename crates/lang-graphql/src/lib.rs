//! GraphQL (executable documents and schema definition language): tree-sitter parses (through
//! `core-treesitter`, mapping in `mapping.rs`), `format.rs` lays the definitions out like Prettier.

core_syntax::include_generated!();
mod format;
mod hooks;
mod mapping;
mod rules;

pub use format::{GraphqlFormatter, option_schema};

include!("provider.generated.rs");
