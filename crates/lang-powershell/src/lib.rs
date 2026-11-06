//! PowerShell: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`), the
//! formatter in `format.rs` re-indents the lines and adds the blanks and breaks
//! PSScriptAnalyzer's `Invoke-Formatter` adds.

core_syntax::include_generated!();
mod format;
mod mapping;
mod rules;

pub use format::{PowerShellFormatter, option_schema};

use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::with_composite(
        Spec {
            language: || {
                core_packs::language!(
                    "powershell",
                    woodruffw_trough_tree_sitter_powershell::LANGUAGE
                )
            },
            grammar: grammars::POWERSHELL,
            mapping: &mapping::MAPPING,
            extensions: &["ps1", "psm1", "psd1"],
            file_names: &[],
            schema: option_schema(),
            formatter: &PowerShellFormatter,
            hooks: &core_rules::DEFAULT_HOOKS,
            rules: rules::RULES,
            pattern_files: provider_api::pattern_files!(),
            families: &[],
            validate: None,
        },
        Some(mapping::COMPOSITE),
    )
}
