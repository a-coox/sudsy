//! Dockerfile and Containerfile: tree-sitter parses (through `core-treesitter`, mapping in
//! `mapping.rs`), the formatter in `format.rs` re-lays each instruction on its source tokens.

core_syntax::include_generated!();
mod embedded;
mod format;
mod mapping;
mod rules;

pub use format::{DockerfileFormatter, option_schema};

include!("provider.generated.rs");
