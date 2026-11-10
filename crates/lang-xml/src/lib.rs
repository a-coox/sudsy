//! XML: tree-sitter parses (through `core-treesitter`, mapping in `lang.yaml`), `validate.rs`
//! checks well-formedness beyond the grammar, formatting is in `format.rs`.

mod format;
mod rules;
mod validate;

pub use format::{XmlFormatter, option_schema};

include!("provider.generated.rs");
