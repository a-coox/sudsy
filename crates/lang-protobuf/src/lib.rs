//! Protobuf (proto2, proto3, editions): tree-sitter parses (through `core-treesitter`, mapping in
//! `lang.yaml`), `format.rs` lays the statements out like `buf format`.

mod format;
mod rules;
mod sort;

pub use format::{ProtoFormatter, option_schema};

include!("provider.generated.rs");
