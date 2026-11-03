//! From the resolved rule configuration of a file to the rules that run on it.

use crate::diagnostic::Severity;
use crate::engine::ActiveRule;
use crate::options::RuleOptions;
use crate::rule::{Layer, Rule};
use core_config::{ConfigError, ResolvedRule};
use std::collections::BTreeMap;

/// The rules among `rules` that are on for a file: configured or default level not `off`, every
/// required layer available, options valid. A rule whose layers are missing is skipped rather
/// than failing the run (plan.md: rules degrade gracefully).
pub fn activate<'a>(
    rules: impl IntoIterator<Item = &'a dyn Rule>,
    configured: &BTreeMap<String, ResolvedRule>,
    has_layer: impl Fn(Layer) -> bool,
) -> Result<Vec<ActiveRule<'a>>, ConfigError> {
    let mut out = Vec::new();
    for rule in rules {
        let meta = rule.meta();
        let setting = configured.get(meta.id);
        let level = setting.map_or(meta.default_level, |s| s.level);
        let Some(severity) = Severity::of(level) else {
            continue;
        };
        if !meta.requires.iter().all(|&l| has_layer(l)) {
            continue;
        }
        let no_options = BTreeMap::new();
        let options =
            RuleOptions::resolve(meta.options, setting.map_or(&no_options, |s| &s.options))
                .map_err(|e| {
                    let from = setting.map_or(String::new(), |s| format!(" ({})", s.source));
                    ConfigError::new(format!("rule `{}`{from}: {e}", meta.id))
                })?;
        out.push(ActiveRule {
            rule,
            severity,
            options,
        });
    }
    Ok(out)
}
