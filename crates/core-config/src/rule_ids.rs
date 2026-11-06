//! Checks that the rules a config names exist.

use crate::chain::ConfigChain;
use crate::error::{ConfigError, unknown_name};
use crate::model::{Config, LintSection};

impl ConfigChain {
    /// Every rule id in a `lint` section of the config files (top level, language sections and
    /// overrides) must be one of `known`, the ids of the rules of all languages and of the
    /// project's pattern and plugin rules (`plugin/rule`). `known` is called only when a config
    /// names a rule: collecting the ids reads every language's rule files.
    pub fn validate_rule_ids<'k>(
        &self,
        known: impl FnOnce() -> Vec<&'k str>,
    ) -> Result<(), ConfigError> {
        if !self
            .files
            .iter()
            .any(|f| sections(&f.config).iter().any(|l| !l.rules.is_empty()))
        {
            return Ok(());
        }
        let known = known();
        for loaded in &self.files {
            for lint in sections(&loaded.config) {
                check_section(lint, &known).map_err(|e| loaded.attribute(e))?;
            }
        }
        Ok(())
    }
}

fn sections(c: &Config) -> Vec<&LintSection> {
    std::iter::once(&c.lint)
        .chain(c.overrides.iter().map(|o| &o.lint))
        .chain(c.all_sections().map(|(_, s)| &s.value.lint))
        .collect()
}

fn check_section(lint: &LintSection, known: &[&str]) -> Result<(), ConfigError> {
    for (id, rule) in &lint.rules {
        if !known.contains(&id.as_str()) {
            return Err(ConfigError::at(
                rule.referenced.line(),
                rule.referenced.column(),
                unknown_name("rule", id, known.iter().copied()),
            ));
        }
    }
    Ok(())
}
