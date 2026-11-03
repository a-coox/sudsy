//! Output formats for diagnostics: human-readable, JSON, SARIF and GitHub Actions
//! annotations. Rendering is a pure function of the diagnostics, so the formats are snapshot-tested.

use clap::ValueEnum;
use core_rules::{Diagnostic, LineIndex, Severity};
use core_syntax::TextRange;
use serde_json::{Value, json};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Sarif,
    Github,
}

/// The diagnostics of one file, with the text they point into.
pub struct FileDiagnostics {
    /// As printed: relative to the working directory, with `/`.
    pub path: String,
    pub text: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// `(id, description)` of every rule a report can mention, for SARIF's rule table.
pub type Catalog = Vec<(String, String)>;

struct Position {
    line: usize,
    column: usize,
    offset: usize,
}

fn position(file: &FileDiagnostics, lines: &LineIndex, offset: core_syntax::TextSize) -> Position {
    let (line, column) = lines.line_col(&file.text, offset);
    Position {
        line,
        column,
        offset: usize::from(offset),
    }
}

fn span(file: &FileDiagnostics, lines: &LineIndex, range: TextRange) -> (Position, Position) {
    (
        position(file, lines, range.start()),
        position(file, lines, range.end()),
    )
}

pub fn render(format: OutputFormat, files: &[FileDiagnostics], catalog: &Catalog) -> String {
    match format {
        OutputFormat::Human => human(files),
        OutputFormat::Json => json_report(files),
        OutputFormat::Sarif => sarif(files, catalog),
        OutputFormat::Github => github(files),
    }
}

/// `(errors, warnings)`.
pub fn count(files: &[FileDiagnostics]) -> (usize, usize) {
    let all = files.iter().flat_map(|f| &f.diagnostics);
    let errors = all
        .clone()
        .filter(|d| d.severity == Severity::Error)
        .count();
    (errors, all.count() - errors)
}

/// Control characters (ANSI escapes, newlines that would fake a line such as a `::` workflow
/// command) shown as escapes: diagnostic text comes from repo-supplied rules and file names.
fn clean(s: &str) -> std::borrow::Cow<'_, str> {
    let bad = |c: char| c.is_control() && c != '\t';
    if !s.chars().any(bad) {
        return s.into();
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if bad(c) {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
    }
    out.into()
}

fn human(files: &[FileDiagnostics]) -> String {
    let mut out = String::new();
    for file in files {
        let lines = LineIndex::new(&file.text);
        for d in &file.diagnostics {
            let at = position(file, &lines, d.range.start());
            let _ = writeln!(
                out,
                "{}:{}:{}: {}[{}]: {}",
                clean(&file.path),
                at.line,
                at.column,
                d.severity.as_str(),
                d.rule,
                clean(&d.message)
            );
            for r in &d.related {
                let at = position(file, &lines, r.range.start());
                let _ = writeln!(out, "    note: {} ({}:{})", clean(&r.message), at.line, at.column);
            }
            for fix in &d.fixes {
                let _ = writeln!(out, "    fix ({}): {}", fix.kind.as_str(), clean(&fix.title));
            }
        }
    }
    out
}

fn location(file: &FileDiagnostics, lines: &LineIndex, range: TextRange) -> Value {
    let (start, end) = span(file, lines, range);
    json!({
        "start": { "line": start.line, "column": start.column, "offset": start.offset },
        "end": { "line": end.line, "column": end.column, "offset": end.offset },
    })
}

fn json_report(files: &[FileDiagnostics]) -> String {
    let mut diagnostics = Vec::new();
    for file in files {
        let lines = LineIndex::new(&file.text);
        for d in &file.diagnostics {
            let related: Vec<Value> = d
                .related
                .iter()
                .map(|r| json!({ "message": r.message, "range": location(file, &lines, r.range) }))
                .collect();
            let fixes: Vec<Value> = d
                .fixes
                .iter()
                .map(|f| {
                    let edits: Vec<Value> = f
                        .edits
                        .iter()
                        .map(|e| {
                            json!({
                                "start": usize::from(e.range.start()),
                                "end": usize::from(e.range.end()),
                                "text": e.text,
                            })
                        })
                        .collect();
                    json!({ "title": f.title, "kind": f.kind.as_str(), "edits": edits })
                })
                .collect();
            diagnostics.push(json!({
                "path": file.path,
                "rule": d.rule,
                "category": d.category.as_str(),
                "severity": d.severity.as_str(),
                "message": d.message,
                "range": location(file, &lines, d.range),
                "related": related,
                "fixes": fixes,
            }));
        }
    }
    let (errors, warnings) = count(files);
    let report = json!({
        "version": 1,
        "summary": { "errors": errors, "warnings": warnings },
        "diagnostics": diagnostics,
    });
    serde_json::to_string_pretty(&report).expect("a JSON value serializes") + "\n"
}

fn sarif(files: &[FileDiagnostics], catalog: &Catalog) -> String {
    let rules: Vec<Value> = catalog
        .iter()
        .map(|(id, text)| json!({ "id": id, "shortDescription": { "text": text } }))
        .collect();
    let mut results = Vec::new();
    for file in files {
        let lines = LineIndex::new(&file.text);
        for d in &file.diagnostics {
            let (start, end) = span(file, &lines, d.range);
            results.push(json!({
                "ruleId": d.rule,
                "level": match d.severity { Severity::Error => "error", Severity::Warning => "warning" },
                "message": { "text": d.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": file.path },
                        "region": {
                            "startLine": start.line,
                            "startColumn": start.column,
                            "endLine": end.line,
                            "endColumn": end.column,
                        },
                    },
                }],
            }));
        }
    }
    let report = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "sudsy",
                "version": env!("CARGO_PKG_VERSION"),
                "rules": rules,
            } },
            "results": results,
        }],
    });
    serde_json::to_string_pretty(&report).expect("a JSON value serializes") + "\n"
}

/// The escapes GitHub's workflow commands need: in the message, and in a property also `:` and `,`.
fn escape(s: &str, property: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            ':' if property => out.push_str("%3A"),
            ',' if property => out.push_str("%2C"),
            c => out.push(c),
        }
    }
    out
}

fn github(files: &[FileDiagnostics]) -> String {
    let mut out = String::new();
    for file in files {
        let lines = LineIndex::new(&file.text);
        for d in &file.diagnostics {
            let (start, end) = span(file, &lines, d.range);
            let _ = writeln!(
                out,
                "::{} file={},line={},col={},endLine={},endColumn={},title={}::{}",
                d.severity.as_str(),
                clean(&escape(&file.path, true)),
                start.line,
                start.column,
                end.line,
                end.column,
                escape(&d.rule, true),
                clean(&escape(&d.message, false)),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn control_characters_are_shown_as_escapes() {
        assert_eq!(clean("a\u{1b}[31m\n::x\tb"), "a\u{1b}[31m\n::x\tb");
        assert_eq!(clean("plain"), "plain");
    }
}
