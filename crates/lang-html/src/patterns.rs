//! The `attr-pattern` family: the value of an attribute (or the attribute's name) must match the
//! rule's `pattern` option, a regex. Param `attribute`: `id`, `class` (each word), `name` (of
//! form controls) or `attr-name` (every attribute's name). The message has `{value}`.

use crate::dom;
use crate::generated::kind as k;
use core_rules::{Family, Params, Re, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};

pub const FAMILY: Family = Family {
    name: "attr-pattern",
    build,
};

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Id,
    Class,
    Name,
    AttrName,
}

struct Pattern {
    meta: &'static RuleMeta,
    target: Target,
    message: String,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let target = p.choice(
        "attribute",
        true,
        &[
            ("id", Target::Id),
            ("class", Target::Class),
            ("name", Target::Name),
            ("attr-name", Target::AttrName),
        ],
    )?;
    let rule = Pattern {
        meta,
        target,
        message: p.text("message")?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Pattern {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[k::START_TAG]
    }

    fn check(&self, tag: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Ok(re) = Re::lazy(ctx.options.string("pattern")) else {
            return;
        };
        let text = ctx.file.text;
        let Some((element, _)) = dom::tag_name(tag, text) else {
            return;
        };
        let controls = [
            "input", "select", "textarea", "button", "fieldset", "output",
        ];
        for a in dom::attrs(tag, text) {
            let mut check = |value: &str, range: TextRange| {
                if !re.is_match(value) {
                    ctx.report(range, self.message.replace("{value}", value));
                }
            };
            let wanted = match self.target {
                Target::AttrName => {
                    check(&text[a.key], a.key);
                    continue;
                }
                Target::Id => "id",
                Target::Class => "class",
                Target::Name => "name",
            };
            if a.name != wanted || (wanted == "name" && !controls.contains(&element.as_str())) {
                continue;
            }
            let (Some(value), Some(range)) = (a.value, a.value_range) else {
                continue;
            };
            if self.target == Target::Class {
                for word in value.split_ascii_whitespace() {
                    let offset = word.as_ptr() as usize - value.as_ptr() as usize;
                    let at = range.start() + TextSize::try_from(offset).unwrap_or_default();
                    let len = TextSize::try_from(word.len()).unwrap_or_default();
                    check(word, TextRange::at(at, len));
                }
            } else {
                check(value, range);
            }
        }
    }
}
