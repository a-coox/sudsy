//! `[Diagnostics.CodeAnalysis.SuppressMessage('PSRuleName', ...)]` silences the rule for the
//! function or class the attribute is on or in (on a method: its class), else for the file. The PSScriptAnalyzer name maps to the
//! Sudsy rule through the parity catalog. Optional target and scope arguments are not read.

use crate::generated::kind;
use core_rules::{FileContext, RuleHooks};
use core_syntax::{TextRange, TextSize};

const CATALOG: &str = include_str!("../../../docs/parity/psscriptanalyzer.yaml");

pub struct PowerShellHooks;

/// The Sudsy rule the catalog maps the PSScriptAnalyzer rule `name` to: a search for its entry,
/// not a parse of the catalog.
fn rule_for(name: &str) -> Option<&'static str> {
    // The module matches the rule name without regard to case.
    let (_, rule) = CATALOG.split("\n  ").find_map(|e| {
        e.split_once(": rule ")
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
    })?;
    rule.split([' ', '\n']).next()
}

impl RuleHooks for PowerShellHooks {
    fn suppressed_ranges(&self, file: &FileContext<'_>) -> Vec<(String, TextRange)> {
        const MARK: &str = "SuppressMessage";
        let text = file.text;
        let mut out = Vec::new();
        for at in memchr::memmem::find_iter(text.as_bytes(), MARK) {
            let rest = &text[at..];
            let Some(open) = rest.find('(') else { continue };
            let Some(end) = rest.find(']') else { continue };
            let args = &rest[open..end];
            let Some(q) = args.find(['\'', '"']) else {
                continue;
            };
            let quote = args.as_bytes()[q] as char;
            let name = args[q + 1..].split(quote).next().unwrap_or("");
            let Some(rule) = rule_for(name) else { continue };
            let start = TextSize::try_from(at).unwrap();
            let after = TextSize::try_from(at + end + 1).unwrap();
            out.extend(scope(file, start, after).map(|range| (rule.to_string(), range)));
        }
        out
    }
}

/// The function around the attribute or right after it, else the whole file; none for an
/// attribute on a single parameter, which the module does not honor.
fn scope(file: &FileContext<'_>, start: TextSize, after: TextSize) -> Option<TextRange> {
    let mut next = usize::from(after);
    // Stacked attributes and blanks sit between the attribute and what it applies to.
    loop {
        let rest = &file.text[next..];
        let trimmed = rest.trim_start();
        next += rest.len() - trimmed.len();
        match trimmed
            .starts_with('[')
            .then(|| trimmed.find(']'))
            .flatten()
        {
            Some(close) => next += close + 1,
            None => break,
        }
    }
    if file.text[next..].starts_with('$') {
        return None;
    }
    // Functions holding `at`, innermost first: no walk of the whole tree.
    let functions = |at: TextSize| {
        let n = file.root.covering_element(TextRange::empty(at));
        let n = n.as_node().cloned().or_else(|| n.parent());
        n.into_iter()
            .flat_map(|n| n.ancestors())
            .filter(|n| n.kind() == kind::FUNCTION)
    };
    let following = TextSize::try_from(next)
        .ok()
        .and_then(|at| functions(at).find(|f| usize::from(f.text_range().start()) == next));
    functions(start)
        .next()
        .or(following)
        .map(|f| f.text_range())
        .or_else(|| member_body(file, start, next))
        .unwrap_or(file.root.text_range())
        .into()
}

/// The braces of a class, for an attribute on `class Name {` (at `next`) or on a method of it
/// (`[type] Name(...) {`; the module scopes a method's attribute to the whole class). Method
/// parameters holding a `{` are not handled.
fn member_body(file: &FileContext<'_>, start: TextSize, next: usize) -> Option<TextRange> {
    let rest = &file.text[next..];
    let line = rest.split(['{', '\n']).next()?;
    let is_class = line
        .get(..6)
        .is_some_and(|w| w.eq_ignore_ascii_case("class "));
    let is_method =
        line.contains('(') && !line.trim_start().to_ascii_lowercase().starts_with("param");
    // The brace follows on the same line or alone on the next.
    let head = rest[line.len()..].trim_start();
    if !(is_class || is_method) || !head.starts_with('{') {
        return None;
    }
    // The class body is not a node: match the braces by token.
    let at = |pos: TextSize| {
        let n = file
            .root
            .covering_element(TextRange::at(pos, TextSize::new(1)));
        n.into_token()
    };
    let mut tok = if is_class {
        at(TextSize::try_from(file.text.len() - head.len()).ok()?)?
    } else {
        let mut tok = at(start)?;
        let mut depth = 0;
        loop {
            match tok.kind() {
                kind::CLOSE_BRACE => depth += 1,
                kind::OPEN_BRACE | kind::HASH_OPEN if depth > 0 => depth -= 1,
                kind::OPEN_BRACE | kind::HASH_OPEN => break tok,
                _ => {}
            }
            tok = tok.prev_token()?;
        }
    };
    let open = tok.text_range().start();
    let mut depth = 0;
    loop {
        match tok.kind() {
            kind::OPEN_BRACE | kind::HASH_OPEN => depth += 1,
            kind::CLOSE_BRACE => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Some(TextRange::new(open, tok.text_range().end()));
        }
        tok = tok.next_token()?;
    }
}
