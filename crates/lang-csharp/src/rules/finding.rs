//! The `finding` rule family: a rule that reports the places one check of the worker found
//! (`symbol_findings`). Params: `check` (the worker's check name), `message` (`{text}` is the
//! finding's text), `empty` (what `{text}` is when the finding has none, default empty) and
//! `fix-title` (the finding's text replaces its fix range; without a title, or without text,
//! there is no fix).

use super::range_of;
use core_rules::{Edit, Family, FileContext, Params, Rule, RuleContext, RuleMeta};
use core_semantics::Finding;

pub const FAMILY: Family = Family {
    name: "finding",
    build,
};

struct WorkerFinding {
    meta: &'static RuleMeta,
    check: String,
    message: String,
    empty: String,
    fix_title: Option<String>,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let rule = WorkerFinding {
        meta,
        check: p.text("check")?,
        message: p.text("message")?,
        empty: p.optional("empty").unwrap_or_default(),
        fix_title: p.optional("fix-title"),
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for WorkerFinding {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let message = |f: &Finding| {
            let text = if f.text.is_empty() {
                &self.empty
            } else {
                &f.text
            };
            self.message.replace("{text}", text)
        };
        report(file, ctx, &self.check, message, self.fix_title.as_deref());
    }
}

/// Reports every finding of `check`; the fix replaces the finding's fix range (or its span).
pub fn report(
    file: &FileContext<'_>,
    ctx: &mut RuleContext<'_>,
    check: &str,
    message: impl Fn(&Finding) -> String,
    fix: Option<&str>,
) {
    let Some(view) = &file.semantics else { return };
    for f in view.data.answers.findings_of(check) {
        let range = range_of(f.start, f.len);
        let fix = fix.filter(|_| !f.text.is_empty()).map(|label| {
            let target = if f.fix_len == 0 {
                range
            } else {
                range_of(f.fix_start, f.fix_len)
            };
            (label, vec![Edit::replace(target, f.text.clone())])
        });
        ctx.report_fix(range, message(f), fix);
    }
}

#[cfg(test)]
mod tests {
    fn load(family: &str, params: &str) {
        use provider_api::LanguageProvider;
        let source = format!(
            "id: f\nlanguage: csharp\nfamily: {family}\nparams: {{ {params} }}\nmessage: m\nseverity: warn\n"
        );
        let provider = crate::provider_with_command(vec!["no-such-host".into()])
            .with_families(super::super::FAMILIES)
            .with_pattern_rules(Box::leak(Box::new([&*source.leak()])));
        provider.rules();
    }

    #[test]
    #[should_panic(expected = "unknown family `nope`")]
    fn an_unknown_family_is_an_error() {
        load("nope", "");
    }

    #[test]
    #[should_panic(expected = "unknown param `extra`")]
    fn an_unknown_param_is_an_error() {
        load("finding", "check: base_call, extra: x");
    }
}
