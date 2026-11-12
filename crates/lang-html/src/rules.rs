//! The Rust rules of HTML and the families its pattern files name; syntax rules that fit a
//! pattern are files in `rules/`.

use core_rules::{Family, Rule};

pub static RULES: &[&dyn Rule] = crate::document::RULES;

pub static FAMILIES: &[Family] = &[crate::checks::FAMILY, crate::patterns::FAMILY];
