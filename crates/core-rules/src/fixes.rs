//! The fix loop (plan.md "Diagnostics and fixes"): collect fixes, drop overlapping ones by rule
//! priority, apply, re-parse, re-run the rules; repeat until nothing changes or 10 iterations.

use crate::diagnostic::{Diagnostic, Fix};
use core_syntax::TextRange;

/// Most rounds of collect, apply and re-run.
pub const MAX_ITERATIONS: usize = 10;

fn overlaps(a: TextRange, b: TextRange) -> bool {
    if a.is_empty() && b.is_empty() {
        return a.start() == b.start();
    }
    a.start() < b.end() && b.start() < a.end()
}

/// The fixes to apply in one round: for each diagnostic the first fix `applies`, in priority
/// order (category, rule, position); a fix that overlaps an accepted one waits for the next round.
pub fn select(diagnostics: &[Diagnostic], applies: impl Fn(&Fix) -> bool) -> Vec<&Fix> {
    let mut candidates: Vec<(&Diagnostic, &Fix)> = diagnostics
        .iter()
        .filter_map(|d| d.fixes.iter().find(|f| applies(f)).map(|f| (d, f)))
        .collect();
    candidates.sort_by_key(|(d, _)| (d.category, d.rule.clone(), d.range.start()));
    let mut chosen: Vec<&Fix> = Vec::new();
    for (_, fix) in candidates {
        let clash = fix.edits.iter().any(|e| {
            chosen
                .iter()
                .any(|c| c.edits.iter().any(|o| overlaps(e.range, o.range)))
        });
        if !clash {
            chosen.push(fix);
        }
    }
    chosen
}

/// `text` with the edits of `fixes` applied. The fixes must not overlap (see [`select`]).
pub fn apply(text: &str, fixes: &[&Fix]) -> String {
    let mut edits: Vec<_> = fixes.iter().flat_map(|f| &f.edits).collect();
    edits.sort_by_key(|e| (e.range.start(), e.range.end()));
    let mut out = String::with_capacity(text.len());
    let mut at = 0usize;
    for e in edits {
        let (start, end) = (
            u32::from(e.range.start()) as usize,
            u32::from(e.range.end()) as usize,
        );
        out.push_str(&text[at..start]);
        out.push_str(&e.text);
        at = end;
    }
    out.push_str(&text[at..]);
    out
}

/// What one run of the rules found.
pub struct Analysis {
    pub diagnostics: Vec<Diagnostic>,
    /// False when the text does not parse; fixes must not leave the file in that state.
    pub parses: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FixOutcome {
    pub text: String,
    pub applied: usize,
    pub iterations: usize,
    /// Set when a round's result did not parse: that round is undone and the loop stops. Holds the
    /// rules whose fixes were in it.
    pub broke: Option<Vec<String>>,
}

/// Applies fixes of the kinds `applies` accepts until none are left. `analyze` runs the rules on
/// a text.
pub fn fix_loop<E>(
    text: String,
    applies: impl Fn(&Fix) -> bool,
    mut analyze: impl FnMut(&str) -> Result<Analysis, E>,
) -> Result<FixOutcome, E> {
    let mut out = FixOutcome {
        text,
        applied: 0,
        iterations: 0,
        broke: None,
    };
    let mut analysis = analyze(&out.text)?;
    for _ in 0..MAX_ITERATIONS {
        if !analysis.parses {
            break;
        }
        let chosen = select(&analysis.diagnostics, &applies);
        if chosen.is_empty() {
            break;
        }
        let next = apply(&out.text, &chosen);
        if next == out.text {
            break;
        }
        out.iterations += 1;
        let after = analyze(&next)?;
        if !after.parses {
            let mut rules: Vec<String> = analysis
                .diagnostics
                .iter()
                .filter(|d| d.fixes.iter().any(|f| chosen.contains(&f)))
                .map(|d| d.rule.clone())
                .collect();
            rules.dedup();
            out.broke = Some(rules);
            break;
        }
        out.applied += chosen.len();
        out.text = next;
        analysis = after;
    }
    Ok(out)
}
