//! Rules that apply to every language. A language-specific rule cannot reuse a shared rule's name.
//! Those on keys and sequences ([`crate::keys`], [`crate::sorted`]) act on the grammar tags
//! `#mapping` and `#sequence`, so they do nothing in a language that does not tag them.

use crate::comments::{CommentSpace, NoEmptyComment};
use crate::diagnostic::Category;
use crate::keys::{KeyNameCasing, NoDuplicateKeys, NoEmptyKeys};
use crate::options::{Check, Opt, OptionDecl};
use crate::rule::{FileContext, Rule, RuleContext, RuleMeta};
use crate::sorted::{SortedArrayValues, SortedKeys};
use core_config::Level;
use core_syntax::{TextRange, TextSize};

pub static RULES: &[&dyn Rule] = &[
    &MaxLineLength,
    &CommentSpace,
    &NoEmptyComment,
    &NoDuplicateKeys,
    &NoEmptyKeys,
    &KeyNameCasing,
    &SortedKeys,
    &SortedArrayValues,
];

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
    options: &[
        OptionDecl {
            name: "max",
            default: Opt::Int(120),
            check: Check::Any,
            description: "Longest allowed line, in characters.",
        },
        OptionDecl {
            name: "allow_non_breakable_words",
            default: Opt::Bool(true),
            check: Check::Any,
            description: "Do not report a line that has no space after its indentation (and \
                          after a leading `#` run or `- `), such as a long URL.",
        },
    ],
};

impl Rule for MaxLineLength {
    fn meta(&self) -> &'static RuleMeta {
        &MAX_LINE_LENGTH
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let max = usize::try_from(ctx.options.int("max")).unwrap_or(usize::MAX);
        let non_breakable = ctx.options.bool("allow_non_breakable_words");
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
            if non_breakable && is_non_breakable(text) {
                continue;
            }
            let length = text.chars().count();
            ctx.report(
                range,
                format!("line is {length} characters long; the limit is {max}"),
            );
        }
    }
}

/// Does the line have no space after its indentation, a leading `#` run and the character after
/// it, or a leading `-` and the character after it? (yamllint's `allow-non-breakable-words`.)
fn is_non_breakable(line: &str) -> bool {
    let body = line.trim_start_matches(' ');
    let skip = |s: &str| s.chars().skip(1).collect::<String>();
    let rest = match body.chars().next() {
        None => return false,
        Some('#') => skip(body.trim_start_matches('#')),
        Some('-') => body.chars().skip(2).collect(),
        Some(_) => return !body.contains(' '),
    };
    !rest.contains(' ')
}
