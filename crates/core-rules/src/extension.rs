//! Rules that do not come with a provider: declarative pattern rules and Wasm plugin rules
//! They are ordinary [`Rule`]s that the project's
//! config adds to a language's own, so they run through the same engine, configuration,
//! suppressions and fix loop.

use crate::rule::Rule;

/// The language value of a rule that applies to every language.
pub const ALL_LANGUAGES: &str = "*";

/// A rule from a pattern file or a plugin, and the language whose files it runs on.
#[derive(Clone, Copy)]
pub struct ExtensionRule {
    pub language: &'static str,
    pub rule: &'static dyn Rule,
}

/// The extension rules of a project, in load order.
#[derive(Clone, Default)]
pub struct Extensions {
    pub rules: Vec<ExtensionRule>,
    /// Files the rules were read from, so an editor can reload when one changes.
    pub files: Vec<std::path::PathBuf>,
}

impl Extensions {
    /// The rules that run on files of `language`.
    pub fn for_language<'a>(
        &'a self,
        language: &'a str,
    ) -> impl Iterator<Item = &'static dyn Rule> + 'a {
        self.rules
            .iter()
            .filter(move |r| r.language == language || r.language == ALL_LANGUAGES)
            .map(|r| r.rule)
    }

    /// Every rule that runs on files of `language`: the language's `own`, the shared ones and
    /// the project's extension rules for it.
    pub fn with_own<'a>(
        &'a self,
        own: &'a [&'static dyn Rule],
        language: &'a str,
    ) -> impl Iterator<Item = &'static dyn Rule> + 'a {
        own.iter()
            .chain(crate::shared::RULES)
            .copied()
            .chain(self.for_language(language))
    }

    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.rules.iter().map(|r| r.rule.meta().id)
    }
}

/// Equal when they hold the same rule objects: loaders reuse the object for unchanged content,
/// so an unchanged reload does not invalidate any rule result.
impl PartialEq for Extensions {
    fn eq(&self, other: &Self) -> bool {
        self.rules.len() == other.rules.len()
            && self.rules.iter().zip(&other.rules).all(|(a, b)| {
                a.language == b.language
                    && std::ptr::addr_eq(a.rule as *const dyn Rule, b.rule as *const dyn Rule)
            })
    }
}

impl std::fmt::Debug for Extensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.ids()).finish()
    }
}
