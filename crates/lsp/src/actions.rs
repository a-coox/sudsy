//! `textDocument/codeAction` (plan.md "LSP server"): a quick fix per fix of a diagnostic in the
//! range, "fix all of a rule in this file", and "fix all safe problems" as `source.fixAll.sudsy`.

use crate::diagnostics::to_lsp;
use crate::position::{Positions, minimal_edit};
use crate::workspace::Workspace;
use core_incremental::{Db, Failure, File, fix_text, lint, selection, transient};
use core_rules::fixes::{apply, select};
use core_rules::suppress::directive_comment;
use core_rules::{Diagnostic, Fix, FixKind};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Range, TextEdit, Uri, WorkspaceEdit,
};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// The kind of the action that applies every safe fix, for `editor.codeActionsOnSave`.
pub const FIX_ALL_KIND: &str = "source.fixAll.sudsy";

/// Does a request for the kinds `only` (all when `None`) want actions of `kind`? A kind includes
/// the kinds below it: `source.fixAll` asks for `source.fixAll.sudsy`.
fn wanted(only: Option<&[CodeActionKind]>, kind: &CodeActionKind) -> bool {
    only.is_none_or(|only| {
        only.iter().any(|o| {
            kind.as_str() == o.as_str()
                || kind
                    .as_str()
                    .strip_prefix(o.as_str())
                    .is_some_and(|rest| rest.starts_with('.'))
        })
    })
}

fn edit_action(title: String, kind: CodeActionKind, uri: &Uri, edits: Vec<TextEdit>) -> CodeAction {
    CodeAction {
        title,
        kind: Some(kind),
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri.clone(), edits)])),
            ..Default::default()
        }),
        ..Default::default()
    }
}

impl Workspace {
    pub fn code_actions(
        &self,
        path: &Path,
        uri: &Uri,
        range: Range,
        only: Option<&[CodeActionKind]>,
    ) -> Result<Vec<CodeActionOrCommand>, Failure> {
        let Some(file) = self.file(path) else {
            return Ok(Vec::new());
        };
        let linted = lint(&self.db, file).as_ref().map_err(Clone::clone)?;
        if !linted.parses {
            return Ok(Vec::new());
        }
        let text = file.text(&self.db);
        let positions = Positions::new(text);
        let mut actions = Vec::new();

        if wanted(only, &CodeActionKind::QUICKFIX) {
            let (start, end) = (positions.offset(range.start), positions.offset(range.end));
            let touched: Vec<&Diagnostic> = linted
                .diagnostics
                .iter()
                .filter(|d| {
                    usize::from(d.range.start()) <= end && start <= usize::from(d.range.end())
                })
                .collect();
            for d in &touched {
                for (i, fix) in d.fixes.iter().enumerate() {
                    actions.push(quick_fix(d, fix, i == 0, &positions, uri));
                }
            }
            let mut seen = BTreeSet::new();
            for d in &touched {
                let line = text[..d.range.start().into()].matches('\n').count();
                if seen.insert((line, d.rule.as_str())) {
                    actions.extend(self.suppress(d, text, &positions, uri, file));
                }
            }
            let rules: BTreeSet<&str> = touched
                .iter()
                .filter(|d| !d.fixes.is_empty())
                .map(|d| d.rule.as_str())
                .collect();
            for rule in rules {
                actions.extend(self.fix_rule(&linted.diagnostics, rule, &positions, uri, file));
            }
        }

        let fix_all = CodeActionKind::new(FIX_ALL_KIND);
        if wanted(only, &fix_all) {
            let fixed = fix_text(&self.db, file, |fix| fix.kind == FixKind::Safe)?;
            if let Some(edit) = minimal_edit(text, &fixed.text) {
                actions.push(edit_action(
                    "Fix all safe problems in this file".into(),
                    fix_all,
                    uri,
                    vec![edit],
                ));
            }
        }
        Ok(actions
            .into_iter()
            .map(CodeActionOrCommand::CodeAction)
            .collect())
    }

    /// "Suppress `rule` on this line": a `disable-next-line` comment above the diagnostic's line,
    /// at its indentation. `None` when the language has no such comment, or the file would no
    /// longer parse with it (plain JSON has no comments).
    fn suppress(
        &self,
        d: &Diagnostic,
        text: &str,
        positions: &Positions<'_>,
        uri: &Uri,
        file: File,
    ) -> Option<CodeAction> {
        let sel = selection(&self.db, file).as_ref()?;
        let comment = directive_comment(self.db.providers()[sel.provider].grammar(), &d.rule)?;
        let start = text[..d.range.start().into()]
            .rfind('\n')
            .map_or(0, |i| i + 1);
        let indent = &text[start
            ..text[start..]
                .find(|c| c != ' ' && c != '\t')
                .map_or(text.len(), |i| start + i)];
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let inserted = format!("{indent}{comment}{eol}");
        let step = transient(
            &self.db,
            file,
            &format!("{}{inserted}{}", &text[..start], &text[start..]),
        );
        if !lint(&self.db, step).as_ref().is_ok_and(|l| l.parses) {
            return None;
        }
        let at = positions.range(start, start);
        let mut action = edit_action(
            format!("Suppress `{}` on this line", d.rule),
            CodeActionKind::QUICKFIX,
            uri,
            vec![TextEdit::new(at, inserted)],
        );
        action.diagnostics = Some(vec![to_lsp(d, positions, uri)]);
        Some(action)
    }

    /// The first fix of every problem of `rule` in the file, safe or unsafe (the user picked the
    /// rule), in one edit; `None` when there are fewer than two, or the result would not parse.
    fn fix_rule(
        &self,
        diagnostics: &[Diagnostic],
        rule: &str,
        positions: &Positions<'_>,
        uri: &Uri,
        file: File,
    ) -> Option<CodeAction> {
        let of_rule: Vec<Diagnostic> = diagnostics
            .iter()
            .filter(|d| d.rule == rule && !d.fixes.is_empty())
            .cloned()
            .collect();
        let fixes = select(&of_rule, |f| {
            matches!(f.kind, FixKind::Safe | FixKind::Unsafe)
        });
        if fixes.len() < 2 {
            return None;
        }
        let text = file.text(&self.db);
        let fixed = apply(text, &fixes);
        let step = transient(&self.db, file, &fixed);
        if !lint(&self.db, step).as_ref().is_ok_and(|l| l.parses) {
            return None;
        }
        let edit = minimal_edit(text, &fixed)?;
        let mut action = edit_action(
            format!("Fix all `{rule}` problems in this file"),
            CodeActionKind::QUICKFIX,
            uri,
            vec![edit],
        );
        action.diagnostics = Some(of_rule.iter().map(|d| to_lsp(d, positions, uri)).collect());
        Some(action)
    }
}

fn quick_fix(
    d: &Diagnostic,
    fix: &Fix,
    first: bool,
    positions: &Positions<'_>,
    uri: &Uri,
) -> CodeAction {
    let edits = fix
        .edits
        .iter()
        .map(|e| {
            TextEdit::new(
                positions.range(e.range.start().into(), e.range.end().into()),
                e.text.clone(),
            )
        })
        .collect();
    let title = match fix.kind {
        FixKind::Safe => fix.title.clone(),
        FixKind::Unsafe | FixKind::Suggestion => format!("{} (unsafe)", fix.title),
    };
    let mut action = edit_action(title, CodeActionKind::QUICKFIX, uri, edits);
    action.diagnostics = Some(vec![to_lsp(d, positions, uri)]);
    action.is_preferred = Some(first && fix.kind == FixKind::Safe);
    action
}
