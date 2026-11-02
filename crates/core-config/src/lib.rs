//! `sudsy.yaml` loading: parsing and validation with source locations.
//!
//! Presets, `.editorconfig` mapping and the resolution order are M2; this crate only reads the
//! file into the structure of plan.md "Configuration and presets". YAML is read with 1.2
//! semantics (via `serde-saphyr`), so `yes`, `no`, `on` and `off` are plain strings.

mod error;
mod load;
mod model;

pub use error::ConfigError;
pub use load::{CONFIG_FILE_NAMES, discover, load_file, parse_str};
pub use model::{
    Config, LanguageSection, Level, LintSection, OptionValue, Options, Override, Plugins,
    RuleConfig,
};
