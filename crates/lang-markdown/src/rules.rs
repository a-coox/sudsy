//! The Rust rules of Markdown (the heading checks that need the whole document); the rules that
//! fit a pattern are files in `rules/`.

use crate::format::{heading_level, heading_range};
use crate::generated::kind;
use core_rules::rule::FileContext;
use core_rules::{Rule, RuleContext, rule};
use core_syntax::{SyntaxNode, TextRange};

pub static RULES: &[&dyn Rule] = &[&HeadingIncrement, &NoDuplicateHeading, &SingleTitle];

/// Every heading with its level, its text and the range of the text.
fn headings(file: &FileContext<'_>) -> Vec<(usize, String, TextRange)> {
    file.root
        .descendants()
        .filter(|n| n.kind() == kind::HEADING)
        .map(|h: SyntaxNode| {
            let level = heading_level(file.text, &h);
            let text: Vec<String> = h
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| {
                    ![kind::WHITESPACE, kind::NEWLINE, kind::PREFIX, kind::MARKER]
                        .contains(&t.kind())
                })
                .map(|t| t.text().to_string())
                .collect();
            (level, text.join(" "), heading_range(file.text, &h))
        })
        .collect()
}

rule! {
    pub HeadingIncrement {
        id: "heading-increment",
        category: Style,
        default: Warn,
        description: "A heading is more than one level deeper than the heading before it (an `h4` \
                  right after an `h2`). Going back up by any number of levels is fine \
                  (markdownlint MD001).",
        example: "# One\n\n### Three   // reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut previous = 0;
        for (level, _, range) in headings(file) {
            if previous > 0 && level > previous + 1 {
                ctx.report(
                    range,
                    format!("heading level should be {} (it follows level {previous})", previous + 1),
                );
            }
            previous = level;
        }
    }
}

rule! {
    pub NoDuplicateHeading {
        id: "no-duplicate-heading",
        category: Style,
        default: Warn,
        description: "Two headings have the same text (markdownlint MD024), which makes their \
                  anchors ambiguous.",
        example: "## Usage\n\n## Usage   // reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut seen = core_rules::unique::Repeats::default();
        for (_, text, range) in headings(file) {
            let message = || format!("the heading `{text}` is repeated");
            seen.check(ctx, text.clone(), range, message, "first heading with this text");
        }
    }
}

rule! {
    pub SingleTitle {
        id: "single-title",
        category: Style,
        default: Warn,
        description: "A document has more than one top-level heading (markdownlint MD025). The \
                  second and later `h1` are reported.",
        example: "# One\n\n# Two   // reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for (_, _, range) in headings(file).into_iter().filter(|h| h.0 == 1).skip(1) {
            ctx.report(range, "the document has more than one top-level heading");
        }
    }
}
