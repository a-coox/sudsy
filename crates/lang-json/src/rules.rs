//! The rules of JSON; `cargo xtask new-rule json <id> --rust` adds a module under `rules/` and
//! lists it here.

use crate::value_rules;
use core_rules::Rule;

pub static RULES: &[&dyn Rule] = &[&value_rules::UnsafeNumbers];
