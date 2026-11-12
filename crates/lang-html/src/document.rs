//! Rules that read the whole document: ids and references, classes, headings, `<main>`, comments.

use crate::dom;
use crate::generated::kind as k;
use crate::meta;
use core_rules::unique::Repeats;
use core_rules::{FileContext, Rule, RuleContext, rule};
use core_syntax::{TextRange, TextSize};
use std::collections::HashSet;

pub static RULES: &[&dyn Rule] = &[
    &NoDuplicateId,
    &NoDuplicateClass,
    &NoMultipleMain,
    &HeadingLevel,
    &NoMissingReferences,
    &NoRedundantFor,
    &NoConditionalComment,
];

rule! {
    NoDuplicateId {
        id: "no-duplicate-id",
        category: Correctness,
        default: Off,
        description: "An `id` that an earlier element of the document already has. Ids must be \
                  unique: scripts, links and labels find the first element only. Empty ids are \
                  left alone.",
        example: "<p id=\"a\"></p><p id=\"a\"></p>   // reported at the second",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut seen = Repeats::default();
        for tag in dom::start_tags(file.root) {
            for a in dom::attrs(&tag, file.text).into_iter().filter(|a| a.name == "id") {
                let (Some(id), Some(range)) = (a.value, a.value_range) else {
                    continue;
                };
                if !id.is_empty() {
                    seen.check(ctx, id, range, || format!("duplicate id `{id}`"), "first used here");
                }
            }
        }
    }
}

rule! {
    NoDuplicateClass {
        id: "no-duplicate-class",
        category: Suspicious,
        default: Off,
        description: "A class name written twice in one `class` attribute.",
        example: "<p class=\"a b a\">   // reported at the second `a`",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for tag in dom::start_tags(file.root) {
            for a in dom::attrs(&tag, file.text).into_iter().filter(|a| a.name == "class") {
                let (Some(value), Some(range)) = (a.value, a.value_range) else {
                    continue;
                };
                let mut seen = Vec::new();
                for word in value.split_ascii_whitespace() {
                    if seen.contains(&word) {
                        let offset = word.as_ptr() as usize - value.as_ptr() as usize;
                        let at = TextRange::at(
                            range.start() + TextSize::try_from(offset).unwrap_or_default(),
                            TextSize::try_from(word.len()).unwrap_or_default(),
                        );
                        ctx.report(at, format!("class `{word}` is repeated"));
                    }
                    seen.push(word);
                }
            }
        }
    }
}

rule! {
    NoMultipleMain {
        id: "no-multiple-main",
        category: Correctness,
        default: Off,
        description: "A second `<main>` element that is not `hidden`: a page has one main landmark.",
        example: "<main></main><main></main>   // reported at the second",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut first = None;
        for tag in dom::start_tags(file.root) {
            let Some((name, range)) = dom::tag_name(&tag, file.text) else {
                continue;
            };
            if name != "main" || dom::attrs(&tag, file.text).iter().any(|a| a.name == "hidden") {
                continue;
            }
            match first {
                Some(at) => {
                    ctx.report(range, "the page has more than one `<main>`").related(at, "first `<main>`");
                }
                None => first = Some(range),
            }
        }
    }
}

rule! {
    HeadingLevel {
        id: "heading-level",
        category: Correctness,
        default: Off,
        description: "Headings do not follow the document outline: the first heading is not an \
                  `<h1>`, a level is skipped going down the outline (`<h1>` then `<h3>`), or the \
                  page has a second `<h1>`.",
        example: "<h1>A</h1><h3>B</h3>   // reported at the `<h3>`",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let (mut previous, mut h1) = (0, false);
        for tag in dom::start_tags(file.root) {
            let Some((name, range)) = dom::tag_name(&tag, file.text) else {
                continue;
            };
            let level = match name.as_bytes() {
                [b'h', d @ b'1'..=b'6'] => d - b'0',
                _ => continue,
            };
            if previous == 0 && level != 1 {
                ctx.report(range, format!("the first heading is `<{name}>`, not `<h1>`"));
            } else if previous != 0 && level > previous + 1 {
                ctx.report(range, format!("`<{name}>` skips from `<h{previous}>`: a level goes down one at a time"));
            }
            if level == 1 && std::mem::replace(&mut h1, true) {
                ctx.report(range, "the page has more than one `<h1>`");
            }
            previous = level;
        }
    }
}

rule! {
    NoMissingReferences {
        id: "no-missing-references",
        category: Correctness,
        default: Off,
        description: "An attribute that refers to an `id` (`for`, `aria-labelledby`, `list`, `headers`, \
                  `form`, ...) names an id that no element of the document has.",
        example: "<label for=\"x\">   // no element has id=\"x\"",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let tags: Vec<_> = dom::start_tags(file.root)
            .filter_map(|t| Some((dom::tag_name(&t, file.text)?.0, dom::attrs(&t, file.text))))
            .collect();
        let ids: HashSet<_> = tags
            .iter()
            .flat_map(|(_, attrs)| attrs.iter().filter(|a| a.name == "id").filter_map(|a| a.value))
            .collect();
        for (name, attrs) in &tags {
            for a in attrs.iter().filter(|a| meta::flagged("reference", name, &a.name)) {
                let (Some(value), Some(range)) = (a.value, a.value_range) else {
                    continue;
                };
                for id in value.split_ascii_whitespace().filter(|id| !ids.contains(id)) {
                    let offset = id.as_ptr() as usize - value.as_ptr() as usize;
                    let at = TextRange::at(
                        range.start() + TextSize::try_from(offset).unwrap_or_default(),
                        TextSize::try_from(id.len()).unwrap_or_default(),
                    );
                    ctx.report(at, format!("no element has the id `{id}`"));
                }
            }
        }
    }
}

rule! {
    NoRedundantFor {
        id: "no-redundant-for",
        category: Style,
        default: Off,
        description: "A `<label>` has a `for` that names a control inside it, which the label \
                  already labels.",
        example: "<label for=\"x\"><input id=\"x\"></label>   <label><input></label>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for label in dom::start_tags(file.root) {
            if dom::tag_name(&label, file.text).is_none_or(|(n, _)| n != "label") {
                continue;
            }
            let attrs = dom::attrs(&label, file.text);
            let Some((for_attr, target)) = attrs.iter().find_map(|a| Some((a, a.value.filter(|_| a.name == "for")?))) else {
                continue;
            };
            let Some(element) = label.parent() else {
                continue;
            };
            let holds = dom::start_tags(&element).skip(1).any(|t| {
                dom::attrs(&t, file.text).iter().any(|a| a.name == "id" && a.value == Some(target))
            });
            if holds {
                ctx.report(for_attr.key, "the `for` is redundant: the label holds the control");
            }
        }
    }
}

rule! {
    NoConditionalComment {
        id: "no-conditional-comment",
        category: Suspicious,
        default: Off,
        description: "An Internet Explorer conditional comment (`<!--[if IE]>`). Only IE 9 and \
                  older read them; other browsers see a comment.",
        example: "<!--[if IE]><p>old</p><![endif]-->",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for t in file.root.descendants_with_tokens().filter_map(|e| e.into_token()) {
            if t.kind() == k::COMMENT
                && (file.text[t.text_range()].starts_with("<!--[if")
                    || file.text[t.text_range()].starts_with("<!--<![endif]"))
            {
                ctx.report(t.text_range(), "conditional comments are only read by old Internet Explorer");
            }
        }
    }
}
