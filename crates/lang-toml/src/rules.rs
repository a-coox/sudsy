//! The rules of TOML; `cargo xtask new-rule toml <id> --rust` adds a module under `rules/` and
//! lists it here.

use core_rules::Rule;

pub static RULES: &[&dyn Rule] = &[];
