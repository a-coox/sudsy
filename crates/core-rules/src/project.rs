//! Project scope (plan.md "Rule scopes"): rules gather facts from each file, then report across
//! files once all files have been seen.

use crate::diagnostic::{Diagnostic, Severity};
use crate::options::RuleOptions;
use crate::rule::RuleMeta;
use core_syntax::TextRange;
use std::path::{Path, PathBuf};

/// One fact a rule gathered from a file. Facts are plain strings so that they can be cached with
/// the file's result and compared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub rule: &'static str,
    pub file: PathBuf,
    pub range: TextRange,
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectFacts {
    facts: Vec<Fact>,
    current: Option<(&'static str, PathBuf)>,
}

impl ProjectFacts {
    /// Records a fact for the rule and file being collected.
    pub fn add(&mut self, range: TextRange, key: impl Into<String>, value: impl Into<String>) {
        let (rule, file) = self
            .current
            .clone()
            .expect("facts are added from `Rule::collect` only");
        self.facts.push(Fact {
            rule,
            file,
            range,
            key: key.into(),
            value: value.into(),
        });
    }

    pub(crate) fn begin(&mut self, rule: &'static str, file: &Path) {
        self.current = Some((rule, file.to_path_buf()));
    }

    pub(crate) fn end(&mut self) {
        self.current = None;
    }

    /// The facts of one rule, in the order the files were added.
    pub fn of_rule<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Fact> {
        self.facts.iter().filter(move |f| f.rule == id)
    }

    pub fn extend(&mut self, other: &ProjectFacts) {
        self.facts.extend(other.facts.iter().cloned());
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }
}

pub struct ProjectContext<'a> {
    pub options: &'a RuleOptions,
    pub(crate) meta: &'static RuleMeta,
    pub(crate) severity: Severity,
    pub(crate) out: &'a mut Vec<(PathBuf, Diagnostic)>,
}

impl ProjectContext<'_> {
    pub fn report(
        &mut self,
        file: &Path,
        range: TextRange,
        message: impl Into<String>,
    ) -> &mut Diagnostic {
        self.out.push((
            file.to_path_buf(),
            Diagnostic {
                rule: self.meta.id.to_string(),
                category: self.meta.category,
                severity: self.severity,
                message: message.into(),
                range,
                related: Vec::new(),
                fixes: Vec::new(),
            },
        ));
        &mut self.out.last_mut().expect("just pushed").1
    }
}
