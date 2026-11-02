//! The structure of `sudsy.yaml` (plan.md "Configuration and presets"), read with source
//! locations so later checks can point at the offending value.

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_saphyr::Spanned;
use std::collections::BTreeMap;
use std::fmt;

/// A format or rule option value. Format options are checked against a provider's declaration
/// ([`crate::OptionSchema`]); rule options are stored as data for the rule engine.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OptionValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<OptionValue>),
    Map(BTreeMap<String, OptionValue>),
}

/// Option values by name, each with the position of its value in the file.
pub type Options = BTreeMap<String, Spanned<OptionValue>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Off,
    Warn,
    Error,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Off => "off",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// `rule: warn` or `rule: { level: warn, options: { .. } }`. The options are data until the rule
/// engine (which knows each rule's option struct) checks them.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleConfig {
    pub level: Level,
    pub options: BTreeMap<String, OptionValue>,
}

impl<'de> Deserialize<'de> for RuleConfig {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = RuleConfig;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(
                    "a level (off, warn, error) or a map with `level` and optional `options`",
                )
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<RuleConfig, E> {
                let level = match v {
                    "off" => Level::Off,
                    "warn" => Level::Warn,
                    "error" => Level::Error,
                    other => return Err(E::unknown_variant(other, &["off", "warn", "error"])),
                };
                Ok(RuleConfig {
                    level,
                    options: BTreeMap::new(),
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RuleConfig, A::Error> {
                let (mut level, mut options) = (None, BTreeMap::new());
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "level" => level = Some(map.next_value::<Level>()?),
                        "options" => options = map.next_value()?,
                        other => {
                            return Err(de::Error::unknown_field(other, &["level", "options"]));
                        }
                    }
                }
                let level = level.ok_or_else(|| de::Error::missing_field("level"))?;
                Ok(RuleConfig { level, options })
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LintSection {
    /// Rule id (`no-empty-catch`, or `plugin/rule`) to level and options.
    pub rules: BTreeMap<String, Spanned<RuleConfig>>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Plugins {
    /// Globs of pattern-rule files, relative to the config file.
    pub patterns: Vec<Spanned<String>>,
    /// Wasm plugin files, relative to the config file.
    pub wasm: Vec<Spanned<String>>,
}

/// The `csharp:`, `json:` and `yaml:` sections.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LanguageSection {
    pub format: Options,
    pub lint: LintSection,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    /// Glob patterns, at least one.
    pub files: Spanned<Vec<Spanned<String>>>,
    #[serde(default)]
    pub format: Options,
    #[serde(default)]
    pub lint: LintSection,
    #[serde(default)]
    pub csharp: LanguageSection,
    #[serde(default)]
    pub json: LanguageSection,
    #[serde(default)]
    pub yaml: LanguageSection,
}

impl Override {
    pub fn language_section(&self, language: &str) -> Option<&LanguageSection> {
        match language {
            "csharp" => Some(&self.csharp),
            "json" => Some(&self.json),
            "yaml" => Some(&self.yaml),
            _ => None,
        }
    }
}

/// Analysis depth (plan.md "Analysis depth"): what rules may use. Formatting ignores it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Depth {
    /// Syntax only.
    Fast,
    /// Binding, types, values and flow within each file.
    #[default]
    Standard,
    /// Adds project-scope rules.
    Deep,
}

impl Depth {
    pub fn as_str(self) -> &'static str {
        match self {
            Depth::Fast => "fast",
            Depth::Standard => "standard",
            Depth::Deep => "deep",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Presets (`preset:csharp/microsoft@1`) or config files (paths relative to this file).
    pub extends: Vec<Spanned<String>>,
    /// Files to format and lint (globs relative to this file); everything when empty.
    pub include: Vec<Spanned<String>>,
    /// Files to leave alone (globs relative to this file), in addition to ignore files.
    pub exclude: Vec<Spanned<String>>,
    /// Analysis depth for rules; `standard` when unset.
    pub analysis: Option<Depth>,
    /// Shared format options (every language).
    pub format: Options,
    pub lint: LintSection,
    pub plugins: Plugins,
    pub csharp: LanguageSection,
    pub json: LanguageSection,
    pub yaml: LanguageSection,
    pub overrides: Vec<Override>,
}

/// The languages that have a section, as their grammar names.
pub const LANGUAGES: [&str; 3] = ["csharp", "json", "yaml"];

impl Config {
    /// The `csharp:`, `json:` or `yaml:` section for a grammar's language name.
    pub fn language_section(&self, language: &str) -> Option<&LanguageSection> {
        match language {
            "csharp" => Some(&self.csharp),
            "json" => Some(&self.json),
            "yaml" => Some(&self.yaml),
            _ => None,
        }
    }
}
