//! The shared test helper: every formatter test, in every language crate, goes through
//! [`check`], which enforces the four automatic checks of docs/development/testing.md that
//! concern formatting: lossless round-trip, unchanged meaning, idempotency, and (here) that no
//! comment was lost or reordered.

use crate::safety::equal_ignoring_trivia;
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, print};

/// What a language provides to be tested.
pub trait Subject {
    fn grammar(&self) -> &Grammar;
    /// `Err` with a message when the text has parse errors.
    fn parse(&self, text: &str) -> Result<SyntaxNode, String>;
    fn format(&self, text: &str) -> Result<String, String>;
    /// A comment as it is compared before and after formatting; a language whose formatter
    /// rewrites comment text maps both to the same value.
    fn comment_key(&self, text: &str) -> String {
        text.to_string()
    }
}

/// The comments of a tree in order. Structured trivia (doc comments, directives) count as one,
/// compared line by line without indentation because the printer re-indents them.
fn comments(subject: &dyn Subject, tree: &SyntaxNode) -> Vec<String> {
    let g = subject.grammar();
    let mut out = Vec::new();
    let mut walk = tree.preorder_with_tokens();
    while let Some(event) = walk.next() {
        let rowan::WalkEvent::Enter(e) = event else {
            continue;
        };
        let structured = matches!(e, SyntaxElement::Node(_));
        if g.has_tag(e.kind(), "comment") || g.has_tag(e.kind(), "verbatim-trivia") {
            let text = match &e {
                SyntaxElement::Token(t) => t.text().to_string(),
                SyntaxElement::Node(n) => n.text().to_string(),
            };
            // The printer writes LF only, even inside block comments.
            let text = text.replace("\r\n", "\n");
            out.push(if structured {
                text.lines().map(str::trim).collect::<Vec<_>>().join("\n")
            } else {
                subject.comment_key(text.trim_end())
            });
            if structured {
                walk.skip_subtree();
            }
        }
    }
    out
}

/// Formats `input` and checks the result; returns it for snapshotting.
pub fn check(subject: &dyn Subject, input: &str) -> Result<String, String> {
    let g = subject.grammar();
    let before = subject.parse(input).map_err(|e| format!("input: {e}"))?;
    if print(&before) != input {
        return Err("the input tree does not print back to the input".into());
    }
    let output = subject.format(input).map_err(|e| format!("format: {e}"))?;
    let after = subject
        .parse(&output)
        .map_err(|e| format!("the output does not parse: {e}\n{output}"))?;
    if print(&after) != output {
        return Err("the output tree does not print back to the output".into());
    }
    equal_ignoring_trivia(g, &before, &after).map_err(|e| format!("meaning changed: {e}"))?;
    if comments(subject, &before) != comments(subject, &after) {
        return Err(format!(
            "comments changed: {:?} became {:?}",
            comments(subject, &before),
            comments(subject, &after)
        ));
    }
    let again = subject
        .format(&output)
        .map_err(|e| format!("second format: {e}"))?;
    if again != output {
        return Err(format!(
            "not idempotent:\n--- first\n{output}\n--- second\n{again}"
        ));
    }
    Ok(output)
}

/// [`check`], panicking with the reason.
#[track_caller]
pub fn assert_formats(subject: &dyn Subject, input: &str) -> String {
    match check(subject, input) {
        Ok(out) => out,
        Err(e) => panic!("{e}\n--- input\n{input}"),
    }
}
