//! Dockerfile and Containerfile: tree-sitter parses (through `core-treesitter`, mapping in
//! `lang.yaml`), the formatter in `format.rs` re-lays each instruction on its source tokens.

mod embedded;
mod format;
mod rules;

pub use format::{DockerfileFormatter, option_schema};

include!("provider.generated.rs");
