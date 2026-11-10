//! `[Diagnostics.CodeAnalysis.SuppressMessage('PSRuleName', ...)]` silences the rule for the
//! function the attribute is on or in, else for the file. The PSScriptAnalyzer name maps to the
//! Sudsy rule through the parity catalog. Optional target and scope arguments are not read.

use crate::generated::kind;
use core_rules::{FileContext, RuleHooks};
use core_syntax::{TextRange, TextSize};

const CATALOG: &str = include_str!("../../../docs/parity/psscriptanalyzer.yaml");

pub struct PowerShellHooks;

/// The Sudsy rule the catalog maps the PSScriptAnalyzer rule `name` to: a search for its entry,
/// not a parse of the catalog.
fn rule_for(name: &str) -> Option<&'static str> {
    let at = CATALOG.find(&format!("\n  {name}: rule "))?;
    let rule = &CATALOG[at + name.len() + 10..];
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
        .map_or(file.root.text_range(), |f| f.text_range())
        .into()
}
