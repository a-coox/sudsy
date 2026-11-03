//! The rule trait and the contexts rules run in (plan.md "Rules, diagnostics and fixes").

use crate::diagnostic::{Category, Diagnostic, FixKind, Severity};
use crate::lines::LineIndex;
use crate::options::{OptionDecl, RuleOptions};
use crate::project::{ProjectContext, ProjectFacts};
use core_config::{Level, RuleOptionSchema, RuleSchema};
use core_semantics::{BodyFlow, SemanticView};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TextRange};
use std::path::Path;

pub use core_semantics::Layer;

/// What `sudsy rules` shows and the engine reads about a rule.
#[derive(Debug)]
pub struct RuleMeta {
    pub id: &'static str,
    pub category: Category,
    pub description: &'static str,
    /// Code that triggers the rule and what it should look like instead, for `sudsy rules <id>`.
    pub example: &'static str,
    /// The level when the config does not mention the rule.
    pub default_level: Level,
    /// The most trusted kind of fix the rule offers, if any.
    pub fix: Option<FixKind>,
    /// Layers without which the rule does not run.
    pub requires: &'static [Layer],
    /// Layers the rule uses when they are available, and does less without (a syntactic
    /// fallback at `fast`).
    pub uses: &'static [Layer],
    pub options: &'static [OptionDecl],
}

impl RuleMeta {
    /// The rule as the config's JSON Schema describes it.
    pub fn schema(&self) -> RuleSchema {
        RuleSchema {
            id: self.id.to_string(),
            description: self.description.to_string(),
            options: self
                .options
                .iter()
                .map(|o| RuleOptionSchema {
                    name: o.name.to_string(),
                    description: o.description.to_string(),
                    default: o.default.value(),
                })
                .collect(),
        }
    }
}

/// One file as rules see it.
pub struct FileContext<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    pub root: &'a SyntaxNode,
    pub grammar: &'a Grammar,
    pub lines: &'a LineIndex,
    /// What the provider's semantic model says about the file, when the file's rules need the
    /// layers and the depth allows them.
    pub semantics: Option<SemanticView<'a>>,
}

/// A method, lambda or local function, with its flow graph when the flow layer is available.
pub struct BodyContext<'a> {
    pub file: &'a FileContext<'a>,
    pub node: &'a SyntaxNode,
    pub flow: Option<BodyFlow<'a>>,
}

/// Where a rule reports to. The rule id, category and severity come from the rule and its
/// configuration, never from the rule's code.
pub struct RuleContext<'a> {
    pub file: &'a FileContext<'a>,
    pub options: &'a RuleOptions,
    pub(crate) meta: &'static RuleMeta,
    pub(crate) severity: Severity,
    pub(crate) out: &'a mut Vec<Diagnostic>,
}

impl<'a> RuleContext<'a> {
    pub fn new(
        file: &'a FileContext<'a>,
        options: &'a RuleOptions,
        meta: &'static RuleMeta,
        severity: Severity,
        out: &'a mut Vec<Diagnostic>,
    ) -> Self {
        RuleContext {
            file,
            options,
            meta,
            severity,
            out,
        }
    }

    /// Reports a diagnostic; add related spans and fixes to the returned value.
    pub fn report(&mut self, range: TextRange, message: impl Into<String>) -> &mut Diagnostic {
        self.out.push(Diagnostic {
            rule: self.meta.id.to_string(),
            category: self.meta.category,
            severity: self.severity,
            message: message.into(),
            range,
            related: Vec::new(),
            fixes: Vec::new(),
        });
        self.out.last_mut().expect("just pushed")
    }
}

/// A rule. Implement whichever scopes the rule needs; all default to doing nothing. Rules hold
/// no state: options arrive through the context, project facts through [`ProjectFacts`].
pub trait Rule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;

    /// Node scope: the kinds [`Rule::check`] is called for.
    fn kinds(&self) -> &[SyntaxKind] {
        &[]
    }

    fn check(&self, _node: &SyntaxNode, _ctx: &mut RuleContext<'_>) {}

    /// File scope: once per file.
    fn check_file(&self, _file: &FileContext<'_>, _ctx: &mut RuleContext<'_>) {}

    /// Body scope: once per method, lambda or local function of the provider.
    fn check_body(&self, _body: &BodyContext<'_>, _ctx: &mut RuleContext<'_>) {}

    /// Project scope, phase 1: gather facts from each file.
    fn collect(&self, _file: &FileContext<'_>, _facts: &mut ProjectFacts) {}

    /// Project scope, phase 2: report across files.
    fn finish(&self, _facts: &ProjectFacts, _ctx: &mut ProjectContext<'_>) {}
}
