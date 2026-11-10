//! PowerShell: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`), the
//! formatter in `format.rs` re-indents the lines and adds the blanks and breaks
//! PSScriptAnalyzer's `Invoke-Formatter` adds.

mod format;
mod hooks;
mod mapping;
mod rules;

pub use format::{PowerShellFormatter, option_schema};

include!("provider.generated.rs");
