//! Suppression comments, in each grammar's comment syntax, each with an optional rule list (none:
//! every rule): `sudsy-disable-next-line`, `sudsy-disable-line`, `sudsy-disable` ... `sudsy-enable`
//! (a block) and `sudsy-disable-file`. The marker words come from the grammar's `suppress` lines;
//! a grammar without them (and plain JSON, which has no comments) has config-only suppression.

use crate::diagnostic::Diagnostic;
use crate::rule::FileContext;
use core_syntax::{Grammar, TextSize};

/// The role name in `suppress disable-next-line "..."`.
const ROLE: &str = "disable-next-line";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// `disable-next-line`: silences the line after the comment.
    NextLine,
    /// `disable-line`: silences the line the comment ends on.
    Line,
    /// `disable`: from the comment's line on, until an `enable` naming the rule.
    Disable,
    Enable,
    /// `disable-file`: the whole file.
    File,
}

/// Directive roles and what each does.
const ROLES: [(&str, Kind); 5] = [
    (ROLE, Kind::NextLine),
    ("disable-line", Kind::Line),
    ("disable", Kind::Disable),
    ("enable", Kind::Enable),
    ("disable-file", Kind::File),
];

struct Directive {
    kind: Kind,
    /// Zero-based line the comment ends on.
    line: usize,
    /// Empty: every rule.
    rules: Vec<String>,
}

impl Directive {
    fn names(&self, rule: &str) -> bool {
        self.rules.is_empty() || self.rules.iter().any(|r| r == rule)
    }
}

pub struct Suppressions(Vec<Directive>);

impl Suppressions {
    pub fn scan(file: &FileContext<'_>) -> Suppressions {
        // Most files have no directive: one search for the prefix every marker shares is much
        // cheaper than walking every token for comments.
        let mut shared: Option<&str> = None;
        for s in &file.grammar.suppressions {
            if ROLES.iter().any(|r| r.0 == s.role) {
                let m = s.marker.as_str();
                shared = Some(match shared {
                    None => m,
                    Some(p) => &p[..p.bytes().zip(m.bytes()).take_while(|(x, y)| x == y).count()],
                });
            }
        }
        if shared.is_none_or(|p| !file.text.contains(p)) {
            return Suppressions(Vec::new());
        }
        let markers: Vec<(&str, Kind)> = ROLES
            .iter()
            .filter_map(|&(role, kind)| {
                let s = file.grammar.suppressions.iter().find(|s| s.role == role)?;
                Some((s.marker.as_str(), kind))
            })
            .collect();
        let mut out = Vec::new();
        for token in file
            .root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| file.grammar.has_tag(t.kind(), "comment"))
        {
            let body = file.grammar.comment_body(token.text());
            let Some((rest, kind)) = markers.iter().find_map(|&(marker, kind)| {
                let rest = body.strip_prefix(marker)?;
                // The marker is a word: `sudsy-disable-next-lines` is something else.
                (!rest.chars().next().is_some_and(|c| !c.is_whitespace())).then_some((rest, kind))
            }) else {
                continue;
            };
            let rules = rest
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|r| !r.is_empty())
                .map(str::to_string)
                .collect();
            let end = token.text_range().end();
            // A comment that ends with its line break ends on the next line's start.
            let ends_line = token.text().ends_with(['\n', '\r']);
            let line = file.lines.line(end) - usize::from(ends_line);
            out.push(Directive { kind, line, rules });
        }
        Suppressions(out)
    }

    pub fn suppresses(&self, file: &FileContext<'_>, d: &Diagnostic) -> bool {
        self.suppresses_at(file, &d.rule, d.range.start())
    }

    /// Does a directive silence `rule` on the line `at` is on?
    pub fn suppresses_at(&self, file: &FileContext<'_>, rule: &str, at: TextSize) -> bool {
        let line = file.lines.line(at);
        let mut blocked = false;
        for s in self.0.iter().filter(|s| s.names(rule)) {
            match s.kind {
                Kind::NextLine if s.line + 1 == line => return true,
                Kind::Line if s.line == line => return true,
                Kind::File => return true,
                // Directives are in document order: the last one at or above the line wins.
                Kind::Disable if s.line <= line => blocked = true,
                Kind::Enable if s.line <= line => blocked = false,
                _ => {}
            }
        }
        blocked
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
