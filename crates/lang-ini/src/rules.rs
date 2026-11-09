//! The Rust rules of INI; syntax rules that fit a pattern are files in `rules/`.

mod no_duplicate_sections;

use core_rules::Rule;

pub static RULES: &[&dyn Rule] = &[&no_duplicate_sections::NoDuplicateSections];
