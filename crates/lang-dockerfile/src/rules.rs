//! The Rust rules of Dockerfile and the families its pattern files name; rules that fit a pattern
//! are files in `rules/`.

mod run_command;
mod stages;

use core_rules::{Family, Rule};

pub static RULES: &[&dyn Rule] = &[
    &stages::UntaggedBaseImage,
    &stages::LastUserNotRoot,
    &stages::DuplicateStageName,
    &stages::CopyFromUnknownStage,
    &stages::CopyFromOwnStage,
    &stages::FromFirst,
];

pub static FAMILIES: &[Family] = &[run_command::FAMILY, stages::ONCE_PER_STAGE];
