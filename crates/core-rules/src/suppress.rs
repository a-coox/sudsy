//! `sudsy-disable-next-line <rule>[, <rule>...]` comments, in each grammar's comment syntax. The
//! marker word comes from the grammar's `suppress` line; a grammar without one (and plain JSON,
//! which has no comments) has config-only suppression.

use crate::diagnostic::Diagnostic;
use crate::rule::FileContext;
use core_syntax::Grammar;

/// The role name in `suppress disable-next-line "..."`.
const ROLE: &str = "disable-next-line";

struct Directive {
    /// Zero-based line the directive applies to.
    line: usize,
    /// Empty: every rule.
    rules: Vec<String>,
}

pub struct Suppressions(Vec<Directive>);

impl Suppressions {
    pub fn scan(file: &FileContext<'_>) -> Suppressions {
        let Some(marker) = file
            .grammar
            .suppressions
            .iter()
            .find(|s| s.role == ROLE)
            .map(|s| s.marker.as_str())
        else {
            return Suppressions(Vec::new());
        };
        // Most files have no directive: finding the marker in the text is much cheaper than
        // walking every token for comments.
        if !file.text.contains(marker) {
            return Suppressions(Vec::new());
        }
        let mut out = Vec::new();
        for token in file
            .root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| file.grammar.has_tag(t.kind(), "comment"))
        {
            let body = file.grammar.comment_body(token.text());
            let Some(rest) = body.strip_prefix(marker) else {
                continue;
            };
            // The marker is a word: `sudsy-disable-next-lines` is something else.
            if rest.chars().next().is_some_and(|c| !c.is_whitespace()) {
                continue;
            }
            let rules = rest
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|r| !r.is_empty())
                .map(str::to_string)
                .collect();
            let end = token.text_range().end();
            // A comment that ends with its line break ends on the next line's start.
            let ends_line = token.text().ends_with(['\n', '\r']);
            let last = file.lines.line(end) - usize::from(ends_line);
            out.push(Directive {
                line: last + 1,
                rules,
            });
        }
        Suppressions(out)
    }

    pub fn suppresses(&self, file: &FileContext<'_>, d: &Diagnostic) -> bool {
        let line = file.lines.line(d.range.start());
        self.0
            .iter()
            .any(|s| s.line == line && (s.rules.is_empty() || s.rules.contains(&d.rule)))
    }
}

/// The comment that suppresses `rule` on the next line, in the grammar's comment syntax; `None`
/// when the grammar has no marker or no comments.
pub fn directive_comment(grammar: &Grammar, rule: &str) -> Option<String> {
    let marker = &grammar.suppressions.iter().find(|s| s.role == ROLE)?.marker;
    if let Some(prefix) = &grammar.comments.line {
        return Some(format!("{prefix} {marker} {rule}"));
    }
    let (open, close) = grammar.comments.block.as_ref()?;
    Some(format!("{open} {marker} {rule} {close}"))
}
