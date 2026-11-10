//! PowerShell: tree-sitter parses (through `core-treesitter`, mapping in `lang.yaml`), the
//! formatter in `format.rs` re-indents the lines and adds the blanks and breaks
//! PSScriptAnalyzer's `Invoke-Formatter` adds.

mod format;
mod hooks;
mod rules;

pub use format::{PowerShellFormatter, option_schema};

include!("provider.generated.rs");
