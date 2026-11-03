//! Configuration: `sudsy.yaml` and the files it extends,
//! presets, `.editorconfig`, and the resolution of both into the format options and rule levels of
//! one file, with the source of every value.
//!
//! The config is read with `serde-saphyr` with YAML 1.2 semantics (`yes`, `no`, `on` and `off` are
//! plain strings), which gives every value its line and column for error messages; see
//! docs/formatting/yaml.md for why Sudsy's own YAML parser is not used here.

mod chain;
mod editorconfig;
mod error;
mod glob;
mod json_schema;
mod load;
mod model;
mod presets;
mod resolve;
mod rule_ids;
mod schema;

pub use chain::{ConfigChain, Loaded, PluginEntry, PluginKind, PresetRef};
pub use editorconfig::{EditorConfig, EditorConfigValue};
pub use error::{ConfigError, suggest, unknown_name};
pub use glob::Matcher;
pub use json_schema::{RuleOptionSchema, RuleSchema, json_schema};
pub use load::{
    CONFIG_FILE_NAMES, MAX_CONFIG_BYTES, discover, is_rule_id, load_file, parse_str, read_capped,
};
pub use model::{
    Config, Depth, LANGUAGES, LanguageSection, Level, LintSection, OptionValue, Options, Override,
    Plugins, RuleConfig, Semantics,
};
pub use presets::{PRESET_PREFIX, Preset};
pub use resolve::{FileInput, Resolved, ResolvedRule, Source};
pub use schema::{OptionDef, OptionError, OptionKind, OptionSchema, ResolvedOptions, shared};
