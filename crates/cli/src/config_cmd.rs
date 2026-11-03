//! `sudsy config print [<file>]`: the resolved config and where each value came from.

use crate::project::{Project, providers};
use anyhow::{Result, anyhow, bail};
use clap::Subcommand;
use core_config::{
    EditorConfig, FileInput, OptionSchema, OptionValue, PluginKind, Resolved, ResolvedRule, Source,
};
use provider_api::LanguageProvider;
use std::fmt::Write;
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Print the resolved config and the source of each value: for one file (overrides and
    /// .editorconfig apply), or for each language when no file is given.
    Print { file: Option<PathBuf> },
}

pub fn run(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Print { file } => print(file.as_deref()),
    }
}

fn print(file: Option<&Path>) -> Result<()> {
    let providers = providers();
    let project = Project::load(&providers)?;
    let mut out = String::new();
    header(&mut out, &project);
    match file {
        Some(file) => {
            let path = core_workspace::absolute(file)?;
            let Some((index, _)) = provider_api::select_provider_index(&providers, &path) else {
                bail!("{}: no language handles this file", project.show(&path));
            };
            let _ = writeln!(out, "\nfile: {}", project.show(&path));
            language(&mut out, &project, providers[index].as_ref(), Some(&path))?;
        }
        None => {
            for provider in &providers {
                let _ = writeln!(out);
                language(&mut out, &project, provider.as_ref(), None)?;
            }
        }
    }
    print!("{out}");
    Ok(())
}

fn header(out: &mut String, project: &Project) {
    let chain = &project.chain;
    if chain.files.iter().all(|f| f.file.is_none()) {
        let _ = writeln!(out, "config: none found (defaults)");
    } else {
        let files: Vec<String> = chain
            .files
            .iter()
            .filter_map(|f| f.file.as_deref())
            .map(|p| project.show(p))
            .collect();
        let _ = writeln!(out, "config: {}", files.join(", "));
    }
    let presets: Vec<&str> = chain.presets.iter().map(|p| p.name.as_str()).collect();
    if !presets.is_empty() {
        let _ = writeln!(out, "presets: {}", presets.join(", "));
    }
    let _ = writeln!(out, "analysis: {}", chain.depth().as_str());
    for (label, patterns) in [("include", true), ("exclude", false)].map(|(label, include)| {
        let patterns: Vec<&str> = chain
            .files
            .iter()
            .flat_map(|f| {
                let list = if include {
                    &f.config.include
                } else {
                    &f.config.exclude
                };
                list.iter().map(|g| g.value.as_str())
            })
            .collect();
        (label, patterns)
    }) {
        if !patterns.is_empty() {
            let _ = writeln!(out, "{label}: {}", patterns.join(", "));
        }
    }
    for entry in chain.plugin_entries() {
        let kind = match entry.kind {
            PluginKind::Pattern => "plugin patterns",
            PluginKind::Wasm => "plugin wasm",
        };
        let _ = writeln!(out, "{kind}: {}", entry.entry);
    }
}

fn language(
    out: &mut String,
    project: &Project,
    provider: &dyn LanguageProvider,
    file: Option<&Path>,
) -> Result<()> {
    let name = provider.grammar().language.as_str();
    let editorconfig = file.map(EditorConfig::for_file);
    let mut input = FileInput::language(name);
    if let Some(path) = file {
        input = input.with_path(path);
    }
    if let Some(ec) = &editorconfig {
        input = input.with_editorconfig(ec);
    }
    let resolved = project
        .chain
        .format_options(&input, provider.option_schema())
        .map_err(|e| anyhow!("{}", crate::project::describe_error(&e, &project.cwd)))?;
    let _ = writeln!(out, "{name} format options:");
    options_table(out, project, provider.option_schema(), &resolved);
    let rules = project
        .chain
        .rules(&input, provider.option_schema())
        .map_err(|e| anyhow!("{}", crate::project::describe_error(&e, &project.cwd)))?;
    if !rules.is_empty() {
        let _ = writeln!(out, "{name} rules:");
        rules_table(out, project, &rules);
    }
    Ok(())
}

fn show_value(v: &OptionValue) -> String {
    match v {
        OptionValue::Bool(b) => b.to_string(),
        OptionValue::Int(n) => n.to_string(),
        OptionValue::Float(f) => f.to_string(),
        OptionValue::String(s) => s.clone(),
        OptionValue::List(items) => format!(
            "[{}]",
            items.iter().map(show_value).collect::<Vec<_>>().join(", ")
        ),
        OptionValue::Map(m) => format!(
            "{{{}}}",
            m.iter()
                .map(|(k, v)| format!("{k}: {}", show_value(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn describe(project: &Project, source: &Source) -> String {
    source.describe(&|p| project.show(p))
}

fn options_table(out: &mut String, project: &Project, schema: &OptionSchema, resolved: &Resolved) {
    let rows: Vec<(String, String, String)> = schema
        .defs()
        .iter()
        .filter_map(|def| {
            let value = resolved.options.get(def.name)?;
            let source = resolved.sources.get(def.name)?;
            Some((
                def.name.to_string(),
                show_value(value),
                describe(project, source),
            ))
        })
        .collect();
    table(out, &rows);
}

fn rules_table(
    out: &mut String,
    project: &Project,
    rules: &std::collections::BTreeMap<String, ResolvedRule>,
) {
    let rows: Vec<(String, String, String)> = rules
        .iter()
        .map(|(id, rule)| {
            let mut value = rule.level.as_str().to_string();
            if !rule.options.is_empty() {
                let options = OptionValue::Map(rule.options.clone());
                let _ = write!(value, " {}", show_value(&options));
            }
            (id.clone(), value, describe(project, &rule.source))
        })
        .collect();
    table(out, &rows);
}

fn table(out: &mut String, rows: &[(String, String, String)]) {
    let name_width = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
    let value_width = rows.iter().map(|r| r.1.len()).max().unwrap_or(0);
    for (name, value, source) in rows {
        let _ = writeln!(
            out,
            "  {name:name_width$}  {value:value_width$}  ({source})"
        );
    }
}
