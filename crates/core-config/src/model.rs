//! The structure of `sudsy.yaml`, read with source
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

/// `semantics:`: facts about methods the analyses use, as
/// qualified names without parameter lists (`MyApp.Log.Fatal` covers every overload).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Semantics {
    /// Methods that never return normally.
    pub does_not_return: Vec<String>,
    /// Methods without side effects.
    pub pure: Vec<String>,
}

/// The `csharp:`, `json:` and `yaml:` sections.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LanguageSection {
    pub format: Options,
    pub lint: LintSection,
    pub semantics: Semantics,
}

/// Language sections by grammar name (`csharp:`, `json:`, ...). Any key that is not a known key
/// is read as one; [`crate::ConfigChain::validate`] checks the names against the providers.
pub type LanguageSections = BTreeMap<String, Spanned<LanguageSection>>;

#[derive(Clone, Debug, PartialEq)]
pub struct Override {
    /// Glob patterns, at least one.
    pub files: Spanned<Vec<Spanned<String>>>,
    pub format: Options,
    pub lint: LintSection,
    pub languages: LanguageSections,
}

impl Override {
    pub fn language_section(&self, language: &str) -> Option<&LanguageSection> {
        self.languages.get(language).map(|s| &s.value)
    }
}

const OVERRIDE_KEYS: &[&str] = &["files", "format", "lint"];

impl<'de> Deserialize<'de> for Override {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Override;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an override: `files` and the sections it changes")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Override, A::Error> {
                let (mut files, mut format, mut lint) =
                    (None, Options::new(), LintSection::default());
                let mut languages = LanguageSections::new();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "files" => files = Some(map.next_value()?),
                        "format" => format = map.next_value()?,
                        "lint" => lint = map.next_value()?,
                        _ => {
                            let section = language_section(&mut map, &key, OVERRIDE_KEYS)?;
                            languages.insert(key, section);
                        }
                    }
                }
                let files = files.ok_or_else(|| de::Error::missing_field("files"))?;
                Ok(Override {
                    files,
                    format,
                    lint,
                    languages,
                })
            }
        }
        d.deserialize_map(V)
    }
}

/// The value of `key`, which is not one of `known`, as a language section. A key close to a known
/// one, or a value that is not a mapping, is an unknown key instead (with a suggestion).
fn language_section<'de, A: MapAccess<'de>>(
    map: &mut A,
    key: &str,
    known: &'static [&'static str],
) -> Result<Spanned<LanguageSection>, A::Error> {
    if crate::error::suggest(key, known.iter().copied()).is_some() {
        return Err(de::Error::unknown_field(key, known));
    }
    // serde gives no typed "not a mapping" error: recognise serde-saphyr's wording for it.
    map.next_value().map_err(|e: A::Error| {
        let m = e.to_string();
        if m.contains("invalid type") || m.contains("expected mapping") {
            de::Error::unknown_field(key, known)
        } else {
            e
        }
    })
}

/// Analysis depth: what rules may use. Formatting ignores it.
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    /// Presets (`preset:csharp/microsoft`) or config files (paths relative to this file).
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
    pub languages: LanguageSections,
    pub overrides: Vec<Override>,
}

const CONFIG_KEYS: &[&str] = &[
    "extends",
    "include",
    "exclude",
    "analysis",
    "format",
    "lint",
    "plugins",
    "overrides",
];

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Config;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a sudsy.yaml mapping")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Config, A::Error> {
                let mut c = Config::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "extends" => c.extends = map.next_value()?,
                        "include" => c.include = map.next_value()?,
                        "exclude" => c.exclude = map.next_value()?,
                        "analysis" => c.analysis = map.next_value()?,
                        "format" => c.format = map.next_value()?,
                        "lint" => c.lint = map.next_value()?,
                        "plugins" => c.plugins = map.next_value()?,
                        "overrides" => c.overrides = map.next_value()?,
                        _ => {
                            let section = language_section(&mut map, &key, CONFIG_KEYS)?;
                            c.languages.insert(key, section);
                        }
                    }
                }
                Ok(c)
            }
        }
        d.deserialize_map(V)
    }
}

impl Config {
    /// The section named after a grammar's language (`csharp:`), if the file has one.
    pub fn language_section(&self, language: &str) -> Option<&LanguageSection> {
        self.languages.get(language).map(|s| &s.value)
    }

    /// Every language section of the file and its overrides, with its name.
    pub fn all_sections(&self) -> impl Iterator<Item = (&String, &Spanned<LanguageSection>)> {
        self.languages
            .iter()
            .chain(self.overrides.iter().flat_map(|o| o.languages.iter()))
    }
}
