//! Markdown: tree-sitter-md's block grammar parses, its inline grammar is injected into the
//! `inline` and `pipe_table_cell` nodes (`core-treesitter` composite, mapping in `mapping.rs`).

core_syntax::include_generated!();
mod format;
mod mapping;
mod rules;

pub use format::{MarkdownFormatter, option_schema};

include!("provider.generated.rs");
