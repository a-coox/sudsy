//! Dockerfile and Containerfile: tree-sitter parses (through `core-treesitter`, mapping in
//! `mapping.rs`), the formatter in `format.rs` re-lays each instruction on its source tokens.

core_syntax::include_generated!();
mod format;
mod mapping;
mod rules;
mod shell;

pub use format::{DockerfileFormatter, option_schema};

use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec {
        language: tree_sitter_containerfile::LANGUAGE.into(),
        grammar: grammars::DOCKERFILE,
        mapping: &mapping::MAPPING,
        extensions: &["dockerfile"],
        file_names: &[
            "Dockerfile",
            "Containerfile",
            "Dockerfile.*",
            "Containerfile.*",
        ],
        schema: option_schema(),
        formatter: &DockerfileFormatter,
        hooks: &core_rules::DEFAULT_HOOKS,
        rules: rules::RULES,
        pattern_files: provider_api::pattern_files!(),
        families: rules::FAMILIES,
        validate: None,
    })
}
