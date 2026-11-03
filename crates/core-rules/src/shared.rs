//! Rules that apply to every language. A language-specific rule cannot reuse a shared rule's name.

use crate::diagnostic::Category;
use crate::options::{Opt, OptionDecl};
use crate::rule::{FileContext, Rule, RuleContext, RuleMeta};
use core_config::Level;
use core_syntax::{TextRange, TextSize};

pub static RULES: &[&dyn Rule] = &[&MaxLineLength];

struct MaxLineLength;

static MAX_LINE_LENGTH: RuleMeta = RuleMeta {
    id: "max-line-length",
    category: Category::Style,
    description: "A line is longer than the configured number of characters.",
    example: "max-line-length: { level: warn, options: { max: 100 } }\n\
              A line of 101 characters is reported from the 101st character on.",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[OptionDecl {
        name: "max",
        default: Opt::Int(120),
        description: "Longest allowed line, in characters.",
    }],
};

impl Rule for MaxLineLength {
    fn meta(&self) -> &'static RuleMeta {
        &MAX_LINE_LENGTH
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let max = usize::try_from(ctx.options.int("max")).unwrap_or(usize::MAX);
        for line in 0..file.lines.line_count() {
            let (start, end) = file.lines.line_range(file.text, line);
            let text = &file.text[start..end];
            let Some((offset, _)) = text.char_indices().nth(max) else {
                continue;
            };
            let range = TextRange::new(
                TextSize::from((start + offset) as u32),
                TextSize::from(end as u32),
            );
            let length = text.chars().count();
            ctx.report(
                range,
                format!("line is {length} characters long; the limit is {max}"),
            );
        }
    }
}
