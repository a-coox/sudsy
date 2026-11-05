//! The Rust rules of EditorConfig; syntax rules that fit a pattern are files in `rules/`.

mod no_duplicate_sections;
mod properties;

use core_rules::Rule;

pub static RULES: &[&dyn Rule] = &[
    &no_duplicate_sections::NoDuplicateSections,
    &properties::UnknownProperty,
    &properties::InvalidPropertyValue,
];
