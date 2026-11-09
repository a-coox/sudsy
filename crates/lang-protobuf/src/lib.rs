//! Protobuf (proto2, proto3, editions): tree-sitter parses (through `core-treesitter`, mapping in
//! `mapping.rs`), `format.rs` lays the statements out like `buf format`.

core_syntax::include_generated!();
mod format;
mod mapping;
mod rules;
mod sort;

pub use format::{ProtoFormatter, option_schema};

include!("provider.generated.rs");
