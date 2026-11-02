//! `sudsy.yaml` loading: parsing and validation with source locations.
//!
//! `.editorconfig` mapping and the full resolution order are M3 (presets are in `presets`); this crate only reads the
//! file into the structure of plan.md "Configuration and presets". YAML is read with 1.2
//! semantics (via `serde-saphyr`), so `yes`, `no`, `on` and `off` are plain strings.

mod error;
mod load;
mod model;
mod presets;
mod schema;

pub use error::ConfigError;
pub use load::{CONFIG_FILE_NAMES, discover, load_file, parse_str};
pub use model::{
    Config, LanguageSection, Level, LintSection, OptionValue, Options, Override, Plugins,
    RuleConfig,
};
pub use presets::{PRESET_PREFIX, Preset, apply_presets};
pub use schema::{
    OptionDef, OptionError, OptionKind, OptionSchema, ResolvedOptions, resolve_format,
    resolve_format_extending, shared,
};
