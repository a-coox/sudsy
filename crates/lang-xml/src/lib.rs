//! XML: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`), `validate.rs`
//! checks well-formedness beyond the grammar, formatting is in `format.rs`.

mod format;
mod mapping;
mod rules;
mod validate;

pub use format::{XmlFormatter, option_schema};

include!("provider.generated.rs");
