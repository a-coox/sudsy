//! [`WasmRule`]: a plugin's rule as a [`Rule`]. The host copies the file to the plugin, the plugin
//! runs the rule with the core's own engine and sends diagnostics back; the host reports them
//! through the rule context, so everything after that (levels, suppressions, fixes) is shared.

use crate::host::{Plugin, PluginError, w};
use core_config::Level;
use core_format::{Directive, NodeKey};
use core_rules::{
    Category, Check, Edit, Fix, FixKind, Layer, Opt, OptionDecl, ProjectContext, ProjectFacts,
    Rule, RuleContext, RuleMeta, RuleOptions,
};
use core_rules::{FileContext, LayoutRole};
use core_syntax::wire::FlatTree;
use core_syntax::{SyntaxKind, TextRange, TextSize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A rule that runs in a Wasm plugin.
pub struct WasmRule {
    plugin: Arc<Plugin>,
    /// The id inside the plugin; the host-side id (`meta().id`) has the plugin's name in front.
    local_id: String,
    meta: &'static RuleMeta,
    role: LayoutRole,
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn bad(plugin: &str, rule: &str, what: &str, value: &str) -> PluginError {
    PluginError(format!(
        "plugin `{plugin}`, rule `{rule}`: unknown {what} `{value}`"
    ))
}

fn layers(plugin: &str, rule: &str, names: &[String]) -> Result<&'static [Layer], PluginError> {
    let parsed: Result<Vec<Layer>, PluginError> = names
        .iter()
        .map(|n| match n.as_str() {
            "binding" => Ok(Layer::Binding),
            "types" => Ok(Layer::Types),
            "values" => Ok(Layer::Values),
            "flow" => Ok(Layer::Flow),
            "project" => Ok(Layer::Project),
            other => Err(bad(plugin, rule, "layer", other)),
        })
        .collect();
    Ok(Box::leak(parsed?.into_boxed_slice()))
}

fn fix_kind(text: &str) -> Option<FixKind> {
    match text {
        "safe" => Some(FixKind::Safe),
        "unsafe" => Some(FixKind::Unsafe),
        "suggestion" => Some(FixKind::Suggestion),
        _ => None,
    }
}

fn meta_of(plugin: &str, r: w::RuleInfo) -> Result<&'static RuleMeta, PluginError> {
    let id = format!("{plugin}/{}", r.id);
    let category = match r.category.as_str() {
        "correctness" => Category::Correctness,
        "suspicious" => Category::Suspicious,
        "style" => Category::Style,
        "cleanup" => Category::Cleanup,
        "performance" => Category::Performance,
        other => return Err(bad(plugin, &r.id, "category", other)),
    };
    let default_level = match r.default_level.as_str() {
        "off" => Level::Off,
        "warn" => Level::Warn,
        "error" => Level::Error,
        other => return Err(bad(plugin, &r.id, "level", other)),
    };
    let fix = match r.fix.as_deref() {
        None => None,
        Some(k) => Some(fix_kind(k).ok_or_else(|| bad(plugin, &r.id, "fix kind", k))?),
    };
    let options: Vec<OptionDecl> = r
        .options
        .iter()
        .map(|o| OptionDecl {
            name: leak(o.name.clone()),
            description: leak(o.description.clone()),
            check: Check::Any,
            default: match &o.default_value {
                w::OptionValue::Flag(b) => Opt::Bool(*b),
                w::OptionValue::Integer(i) => Opt::Int(*i),
                w::OptionValue::Text(s) => Opt::Str(leak(s.clone())),
                w::OptionValue::Texts(l) => Opt::List(Box::leak(
                    l.iter()
                        .map(|s| leak(s.clone()))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )),
            },
        })
        .collect();
    Ok(Box::leak(Box::new(RuleMeta {
        requires: layers(plugin, &r.id, &r.requires)?,
        uses: layers(plugin, &r.id, &r.uses)?,
        id: leak(id),
        category,
        description: leak(r.description),
        example: leak(r.example),
        default_level,
        fix,
        options: Box::leak(options.into_boxed_slice()),
    })))
}

/// The rules of a freshly loaded plugin, as `(language, rule)`.
pub(crate) fn make_rules(
    plugin: Arc<Plugin>,
    info: w::PluginInfo,
) -> Result<Vec<(&'static str, &'static dyn Rule)>, PluginError> {
    let mut out: Vec<(&'static str, &'static dyn Rule)> = Vec::new();
    for r in info.rules {
        // The host checks the language against its providers (`languages::load_extensions`).
        let language: &'static str = match r.language.as_str() {
            "*" => core_rules::ALL_LANGUAGES,
            other => leak(other.to_string()),
        };
        let local_id = r.id.clone();
        let role = match r.layout {
            w::LayoutRole::None => LayoutRole::None,
            w::LayoutRole::Also => LayoutRole::Also,
            w::LayoutRole::Only => LayoutRole::Only,
        };
        let meta = meta_of(&info.name, r)?;
        out.push((
            language,
            Box::leak(Box::new(WasmRule {
                plugin: plugin.clone(),
                local_id,
                meta,
                role,
            })),
        ));
    }
    Ok(out)
}

fn option_list(options: &RuleOptions, decls: &[OptionDecl]) -> Vec<(String, w::OptionValue)> {
    decls
        .iter()
        .map(|d| {
            let value = match d.default {
                Opt::Bool(_) => w::OptionValue::Flag(options.bool(d.name)),
                Opt::Int(_) => w::OptionValue::Integer(options.int(d.name)),
                Opt::Str(_) => w::OptionValue::Text(options.string(d.name).to_string()),
                Opt::List(_) => w::OptionValue::Texts(
                    options.list(d.name).into_iter().map(String::from).collect(),
                ),
            };
            (d.name.to_string(), value)
        })
        .collect()
}

fn file_input(file: &FileContext<'_>) -> w::File {
    let wire = FlatTree::from_tree(file.root).to_wire();
    w::File {
        path: file.path.to_string_lossy().into_owned(),
        language: file.grammar.language.clone(),
        grammar_hash: file.grammar.hash().to_string(),
        text: file.text.to_string(),
        tree_kinds: wire.kinds,
        tree_values: wire.values,
        body_kinds: file.body_kinds.iter().map(|k| k.0).collect(),
        semantics: file
            .semantics
            .as_ref()
            .and_then(|view| rmp_serde::to_vec(&(view.data, view.summaries)).ok()),
    }
}

/// A span the plugin sent, checked against the text: a plugin must not be able to make the host
/// slice a string wrongly.
fn range(text: &str, span: &w::Span) -> Result<TextRange, String> {
    let (start, end) = (span.start as usize, span.end as usize);
    if start > end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return Err(format!(
            "it reported the span {start}..{end}, which is not inside the file"
        ));
    }
    Ok(TextRange::new(
        TextSize::from(span.start),
        TextSize::from(span.end),
    ))
}

fn fix_of(text: &str, f: &w::Fix) -> Result<Fix, String> {
    let kind = fix_kind(&f.kind).ok_or_else(|| format!("it reported the fix kind `{}`", f.kind))?;
    let edits = f
        .edits
        .iter()
        .map(|e| Ok(Edit::replace(range(text, &e.span)?, e.text.clone())))
        .collect::<Result<_, String>>()?;
    Ok(Fix::new(f.title.clone(), kind, edits))
}

impl WasmRule {
    /// Reports what went wrong with the plugin, once per run, and switches it off.
    fn fail(&self, ctx: &mut RuleContext<'_>, reason: String) {
        if ctx.laying_out() {
            self.plugin.disable(reason.clone());
            ctx.layout_failed(format!(
                "plugin `{}` is disabled for this run: {reason}",
                self.plugin.name
            ));
        } else if self.plugin.disable(reason.clone()) {
            ctx.report(
                TextRange::empty(TextSize::from(0)),
                format!(
                    "plugin `{}` is disabled for this run: {reason}",
                    self.plugin.name
                ),
            );
        }
    }

    /// The plugin's layout requests, checked like diagnostics (a bad plugin attaches nothing).
    fn attach(
        &self,
        ctx: &mut RuleContext<'_>,
        found: Vec<w::LayoutRequest>,
    ) -> Result<(), String> {
        let text = ctx.file.text;
        let mut checked = Vec::new();
        for r in &found {
            let span = range(
                text,
                &w::Span {
                    start: r.start,
                    end: r.end,
                },
            )?;
            let at = range(
                text,
                &w::Span {
                    start: r.at,
                    end: r.at,
                },
            )?;
            let directive = Directive::parse(&r.directive)
                .map_err(|e| format!("it requested a layout: {e}"))?;
            let key = NodeKey::new(SyntaxKind(r.kind), span.start().into(), span.end().into());
            checked.push((key, at.start(), directive));
        }
        for (key, at, directive) in checked {
            ctx.attach_at(key, at, directive);
        }
        Ok(())
    }

    fn report(&self, ctx: &mut RuleContext<'_>, found: Vec<w::Diagnostic>) -> Result<(), String> {
        let text = ctx.file.text;
        // Check everything before reporting anything, so a bad plugin adds nothing.
        let mut checked = Vec::new();
        for d in &found {
            let related = d
                .related
                .iter()
                .map(|r| Ok((range(text, &r.span)?, r.message.clone())))
                .collect::<Result<Vec<_>, String>>()?;
            let fixes = d
                .fixes
                .iter()
                .map(|f| fix_of(text, f))
                .collect::<Result<Vec<_>, String>>()?;
            checked.push((range(text, &d.span)?, d.message.clone(), related, fixes));
        }
        for (at, message, related, fixes) in checked {
            let diagnostic = ctx.report(at, message);
            for (range, message) in related {
                diagnostic.related(range, message);
            }
            for fix in fixes {
                diagnostic.fix(fix);
            }
        }
        Ok(())
    }
}

impl Rule for WasmRule {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn layout_role(&self) -> LayoutRole {
        self.role
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        if self.plugin.disabled().is_some() {
            return;
        }
        let options = option_list(ctx.options, self.meta.options);
        let input = file_input(file);
        if ctx.laying_out() {
            let result = self
                .plugin
                .call(|guest, store| guest.call_layout(store, &self.local_id, &options, &input));
            if let Err(reason) = result.and_then(|found| self.attach(ctx, found)) {
                self.fail(ctx, reason);
            }
            return;
        }
        let result = self
            .plugin
            .call(|guest, store| guest.call_check(store, &self.local_id, &options, &input));
        match result.and_then(|found| self.report(ctx, found)) {
            Ok(()) => {}
            Err(reason) => self.fail(ctx, reason),
        }
    }

    fn collect(&self, file: &FileContext<'_>, facts: &mut ProjectFacts) {
        if self.plugin.disabled().is_some() {
            return;
        }
        let input = file_input(file);
        // The options only matter to `check` and `finish`; collecting uses the defaults.
        let result = self
            .plugin
            .call(|guest, store| guest.call_collect(store, &self.local_id, &[], &input));
        match result {
            Ok(found) => {
                for f in found {
                    if let Ok(at) = range(file.text, &f.span) {
                        facts.add(at, f.key, f.value);
                    }
                }
            }
            Err(reason) => {
                self.plugin.disable(reason);
            }
        }
    }

    fn finish(&self, facts: &ProjectFacts, ctx: &mut ProjectContext<'_>) {
        if self.plugin.disabled().is_some() {
            return;
        }
        let input: Vec<w::Fact> = facts
            .of_rule(self.meta.id)
            .map(|f| w::Fact {
                file: f.file.to_string_lossy().into_owned(),
                span: w::Span {
                    start: f.range.start().into(),
                    end: f.range.end().into(),
                },
                key: f.key.clone(),
                value: f.value.clone(),
            })
            .collect();
        let options = option_list(ctx.options, self.meta.options);
        let result = self
            .plugin
            .call(|guest, store| guest.call_finish(store, &self.local_id, &options, &input));
        match result {
            Ok(found) => {
                for d in found {
                    let at = TextRange::new(
                        TextSize::from(d.diagnostic.span.start.min(d.diagnostic.span.end)),
                        TextSize::from(d.diagnostic.span.end),
                    );
                    let file: PathBuf = d.file.into();
                    ctx.report(Path::new(&file), at, d.diagnostic.message);
                }
            }
            Err(reason) => {
                self.plugin.disable(reason);
            }
        }
    }
}
