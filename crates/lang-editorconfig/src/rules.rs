//! The Rust rules of EditorConfig; syntax rules that fit a pattern are files in `rules/`.

mod properties;

use core_rules::Rule;

pub static RULES: &[&dyn Rule] = &[
    &properties::UnknownProperty,
    &properties::InvalidPropertyValue,
];
