use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_saphyr::Spanned;
use std::collections::BTreeMap;
use std::fmt;

/// A format or rule option value. Option names and types are declared by providers (M2), so the
/// config layer keeps them untyped.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OptionValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<OptionValue>),
}

pub type Options = BTreeMap<String, OptionValue>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Off,
    Warn,
    Error,
}

/// `rule: warn` or `rule: { level: warn, options: { .. } }`.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleConfig {
    pub level: Level,
    pub options: Options,
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
                    options: Options::new(),
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RuleConfig, A::Error> {
                let (mut level, mut options) = (None, Options::new());
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
    pub rules: BTreeMap<String, RuleConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Plugins {
    pub patterns: Vec<String>,
    pub wasm: Vec<String>,
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
    /// Glob patterns; must not be empty. Spanned so validation can point at it.
    pub files: Spanned<Vec<String>>,
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

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Presets or config files, e.g. `preset:csharp/microsoft@1`. Resolved in M2.
    pub extends: Vec<Spanned<String>>,
    /// Shared format options (every language).
    pub format: Options,
    pub lint: LintSection,
    pub plugins: Plugins,
    pub csharp: LanguageSection,
    pub json: LanguageSection,
    pub yaml: LanguageSection,
    pub overrides: Vec<Override>,
}
