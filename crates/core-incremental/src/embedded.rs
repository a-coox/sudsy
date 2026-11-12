//! The code of other languages inside a file (see `provider_api::embedded`): linted with the
//! rules and config of its own language, and formatted by its own formatter when the host asks.

use crate::db::Db;
use crate::failure::{Failure, Stage};
use crate::inputs::{ExtensionSet, File};
use crate::queries::{config, source_file};
use core_config::{FileInput, OptionValue, ResolvedOptions};
use core_format::{DirectiveTable, EmbedMode, Embedder};
use core_rules::engine::ActiveRule;
use core_rules::{Diagnostic, Edit, FileContext, LineIndex, activate, run_file};
use core_syntax::SyntaxNode;
use provider_api::embedded::{EmbeddedParse, Resolved, parse_regions, resolve};
use provider_api::{LanguageProvider, SourceFile, format_file, resolve_dialect};

/// Names the host's option that turns the formatting of embedded code off (Prettier's
/// `embeddedLanguageFormatting`: `auto` formats what it knows, `off` keeps all as written).
const FORMATTING: &str = "embedded_language_formatting";

fn lookup<'a>(db: &'a dyn Db, name: &str) -> Resolved<'a> {
    resolve(db.providers().iter().map(|p| p.as_ref()), name)
}

/// The parses of the code of other languages in the file.
pub(crate) fn parse_embedded<'a>(
    db: &'a dyn Db,
    host: &dyn LanguageProvider,
    root: &SyntaxNode,
    text: &str,
) -> Vec<EmbeddedParse<'a>> {
    parse_regions(host, root, text, &|name| lookup(db, name))
}

/// What the rules of each embedded language report, in host offsets. Suppression comments inside
/// the code apply here; the host's apply to the result.
pub(crate) fn lint_embedded(
    db: &dyn Db,
    file: File,
    parses: &[EmbeddedParse<'_>],
) -> Result<Vec<Diagnostic>, Failure> {
    let mut active: Vec<(&str, Vec<ActiveRule<'static>>)> = Vec::new();
    let mut out = Vec::new();
    for e in parses {
        let language = e.provider.language();
        if !active.iter().any(|(l, _)| *l == language) {
            // Read only now: a file with no embedded code does not depend on the config here.
            let config = config(db).as_ref().map_err(Clone::clone)?;
            let extensions = ExtensionSet::get(db).extensions(db);
            let input = FileInput::language(language)
                .with_base(e.provider.config_base())
                .with_path(file.path(db));
            let rules = (config.rules(&input, e.provider.option_schema()))
                .map_err(|e| Failure::new(Stage::Config, &e))?;
            let mut rules = activate(
                extensions.with_own(e.provider.rules(), language),
                &rules,
                |_| false,
            )
            .map_err(|e| Failure::new(Stage::Config, &e))?;
            rules.retain(|a| a.rule.layout_role().reports());
            active.push((language, rules));
        }
        let rules = &active
            .iter()
            .find(|(l, _)| *l == language)
            .expect("just added")
            .1;
        let lines = LineIndex::new(&e.code);
        let context = FileContext {
            path: file.path(db),
            text: &e.code,
            root: &e.root,
            grammar: e.provider.grammar(),
            lines: &lines,
            body_kinds: e.provider.body_kinds(),
            hooks: e.provider.rule_hooks(),
            embedded: &[],
            semantics: None,
        };
        let at = e.region.range.start();
        for mut d in run_file(&context, rules, false).diagnostics {
            d.range += at;
            d.related.iter_mut().for_each(|r| r.range += at);
            for fix in &mut d.fixes {
                fix.edits.iter_mut().for_each(|x| x.range += at);
            }
            d.fixes
                .retain(|f| f.edits.iter().all(|x| keeps_host(&e.region, x)));
            out.push(d);
        }
    }
    Ok(out)
}

/// Does the edit leave the host's parts of the region alone? A line break could take the
/// container prefix of the next line with it.
fn keeps_host(region: &core_syntax::Embedded, edit: &Edit) -> bool {
    region.carved.is_empty()
        || (!edit.text.contains('\n')
            && region
                .carved
                .iter()
                .all(|c| !(c.start() < edit.range.end() && edit.range.start() < c.end())))
}

/// `formatted`, the host's output, with the code of its embedded regions formatted by their own
/// languages and indented as the region is. Code that does not parse or whose language nobody
/// handles stays as written; so does everything when the result would not read back the same.
pub(crate) fn format_embedded(
    db: &dyn Db,
    file: File,
    host: &dyn LanguageProvider,
    options: &ResolvedOptions,
    input_root: &SyntaxNode,
    formatted: String,
) -> String {
    let off = matches!(options.get(FORMATTING), Some(OptionValue::String(s)) if s == "off");
    let regions = |root: &SyntaxNode, text: &str| {
        let mut all = host.embedded(root, text);
        all.retain(|r| r.format);
        all
    };
    if off || !host.formats_embedded() || regions(input_root, file.text(db)).is_empty() {
        return formatted;
    }
    let source = SourceFile {
        text: formatted.clone(),
        ..source_file(db, file, None)
    };
    let Ok(parsed) = host.parse(&source) else {
        return formatted;
    };
    let regions = regions(&parsed.tree, &formatted);
    let crlf = formatted.contains("\r\n");
    let mut out = formatted.clone();
    // From the last region on, so that the offsets of the others stay valid.
    for r in regions.iter().rev() {
        let Some(code) = format_code(db, file, &r.language, &r.stripped(&formatted)) else {
            continue;
        };
        let prefix = prefix_of(r, &formatted);
        let lines: Vec<String> = (code.lines())
            .map(|l| {
                if l.is_empty() {
                    prefix.trim_end().into()
                } else {
                    format!("{prefix}{l}")
                }
            })
            .collect();
        out.replace_range(
            std::ops::Range::<usize>::from(r.range),
            &lines.join(if crlf { "\r\n" } else { "\n" }),
        );
    }
    // The fences must still be there, with the code just written.
    let again = SourceFile {
        text: out.clone(),
        ..source
    };
    match host.parse(&again) {
        Ok(p) if p.errors.is_empty() && regions_agree(host, &p.tree, &out, &regions) => out,
        _ => formatted,
    }
}

/// The host's part of the first line of the region that has code on it: what every line of
/// rewritten code starts with.
fn prefix_of<'a>(region: &core_syntax::Embedded, text: &'a str) -> &'a str {
    let mut at = usize::from(region.range.start());
    for line in text[region.range].split_inclusive('\n') {
        let carved = region.carved.iter().find(|c| usize::from(c.start()) == at);
        let len = carved.map_or(0, |c| usize::from(c.len()));
        if !line[len..].trim().is_empty() {
            return &text[at..at + len];
        }
        at += line.len();
    }
    ""
}

fn regions_agree(
    host: &dyn LanguageProvider,
    root: &SyntaxNode,
    text: &str,
    before: &[core_syntax::Embedded],
) -> bool {
    let after = host.embedded(root, text);
    let after: Vec<_> = after.iter().filter(|r| r.format).collect();
    after.len() == before.len()
        && after
            .iter()
            .zip(before)
            .all(|(a, b)| a.language == b.language)
}

/// The formatted code, if its language is known and it parses; `None` also when nothing changes.
fn format_code(db: &dyn Db, file: File, language: &str, code: &str) -> Option<String> {
    let code = code.replace("\r\n", "\n");
    let out = format_with(db, file, language, EmbedMode::Program, &code, None, false)?;
    (!out.is_empty() && out != code.trim_end_matches('\n')).then_some(out)
}

/// `code` formatted by the provider of `language` (`width` overrides its `line_width`), without
/// the final line break.
fn format_with(
    db: &dyn Db,
    file: File,
    language: &str,
    mode: EmbedMode,
    code: &str,
    width: Option<usize>,
    single_quotes: bool,
) -> Option<String> {
    let (provider, dialect) = lookup(db, language)?;
    let input = FileInput::language(provider.language())
        .with_base(provider.config_base())
        .with_path(file.path(db));
    let mut options = (config(db).as_ref().ok()?)
        .format_options(&input, provider.option_schema())
        .ok()?
        .options;
    let language_version = resolve_dialect(&mut options, dialect);
    if let Some(w) = width {
        options.set("line_width", OptionValue::Int(w.max(20) as i64));
    }
    if single_quotes {
        if options.get("quote_style").is_some() {
            options.set("quote_style", OptionValue::String("single".into()));
        }
        if options.get("single_quote").is_some() {
            options.set("single_quote", OptionValue::Bool(true));
        }
    }
    let mut source = SourceFile::new(format!("embedded.{language}"), 1, code.to_string());
    source.language_version = if mode == EmbedMode::Program {
        language_version
    } else {
        // The provider's parse and layout read the mode: the code is not a whole file.
        options.set("embed_mode", OptionValue::String(mode.name().into()));
        Some(mode.name().into())
    };
    let out = format_file(provider, &source, &options, &DirectiveTable::default()).ok()?;
    Some(out.trim_end_matches('\n').to_string())
}

/// The embedder a host formatter runs under: other languages' code, formatted by their
/// providers with the config of the file.
pub(crate) struct Engine<'a> {
    pub db: &'a dyn Db,
    pub file: File,
}

impl Embedder for Engine<'_> {
    fn format(
        &self,
        language: &str,
        mode: EmbedMode,
        code: &str,
        width: usize,
        single_quotes: bool,
    ) -> Option<String> {
        format_with(
            self.db,
            self.file,
            language,
            mode,
            code,
            Some(width),
            single_quotes,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_syntax::TextSize;

    #[test]
    fn host_parts_are_kept() {
        let region = core_syntax::Embedded {
            language: "x".into(),
            range: core_syntax::TextRange::new(TextSize::from(0), TextSize::from(6)),
            carved: vec![core_syntax::TextRange::new(
                TextSize::from(0),
                TextSize::from(2),
            )],
            format: true,
        };
        let edit = |a: u32, b: u32, t: &str| {
            Edit::replace(core_syntax::TextRange::new(a.into(), b.into()), t)
        };
        assert!(keeps_host(&region, &edit(2, 4, "z")));
        assert!(!keeps_host(&region, &edit(1, 4, "z")));
        assert!(!keeps_host(&region, &edit(2, 4, "\n")));
    }
}
