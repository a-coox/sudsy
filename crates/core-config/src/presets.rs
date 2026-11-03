//! Presets: named config files shipped with a provider,
//! applied by `extends: [preset:csharp/google]` below the shared and language sections.
//!
//! A preset is the text of an ordinary `sudsy.yaml` (`csharp: format: {...}`), read by the same
//! parser, merged and checked like an `extends` file ([`crate::ConfigChain`] expands it). Formatting code never
//! looks at which preset is in use, so a `sudsy.yaml` that copies the file formats exactly the
//! same. Presets are unversioned.

/// `extends` entries that name a preset start with this.
pub const PRESET_PREFIX: &str = "preset:";

/// The language of a preset name: `csharp` for `csharp/google`.
pub(crate) fn language_of(preset: &str) -> &str {
    preset.split('/').next().unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// `language/name`, e.g. `csharp/google`.
    pub name: String,
    /// The text of the config file.
    pub text: &'static str,
}

impl Preset {
    pub fn new(name: &str, text: &'static str) -> Self {
        debug_assert!(name.contains('/') && !name.contains('@'), "{name}");
        Preset {
            name: name.to_string(),
            text,
        }
    }
}
