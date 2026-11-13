//! Rules that read the whole document: ids and references, classes, headings, `<main>`, comments.

use crate::dom;
use crate::generated::kind as k;
use crate::meta;
use core_rules::unique::Repeats;
use core_rules::{FileContext, Rule, RuleContext, rule};
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange, TextSize};
use std::collections::HashSet;

pub static RULES: &[&dyn Rule] = &[
    &NoDuplicateId,
    &NoDuplicateClass,
    &NoMultipleMain,
    &HeadingLevel,
    &NoMissingReferences,
    &NoRedundantFor,
    &NoConditionalComment,
    &InputMissingLabel,
    &MultipleLabeledControls,
    &UniqueLandmark,
    &MapDupName,
    &NoTrailingWhitespace,
    &UnrecognizedCharRef,
];

rule! {
    NoDuplicateId {
        id: "no-duplicate-id",
        category: Correctness,
        default: Warn,
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
                    let offset = word.as_ptr() as usize - value.as_ptr() as usize;
                    if word.starts_with(['{', '}', '%']) || crate::hooks::in_template(&value[..offset]) {
                        continue;
                    }
                    if seen.contains(&word) {
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
        default: Warn,
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
        default: Warn,
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

/// The lowercase name and attributes of every start tag, in document order, with the tag.
type Tag<'a> = (SyntaxNode, String, TextRange, Vec<dom::Attr<'a>>);

fn tags<'a>(file: &FileContext<'a>) -> Vec<Tag<'a>> {
    dom::start_tags(file.root)
        .filter_map(|t| {
            let (name, range) = dom::tag_name(&t, file.text)?;
            let attrs = dom::attrs(&t, file.text);
            Some((t, name, range, attrs))
        })
        .collect()
}

fn attr<'a>(attrs: &[dom::Attr<'a>], name: &str) -> Option<&'a str> {
    attrs.iter().find(|a| a.name == name).and_then(|a| a.value)
}

/// Does the element hold text (a label with none names nothing)?
fn has_text(element: &SyntaxNode) -> bool {
    element.descendants().any(|n| n.kind() == k::CHUNK)
}

/// A control a `<label>` can name: not a hidden or button-like `<input>`.
fn labelable(name: &str, attrs: &[dom::Attr<'_>]) -> bool {
    match name {
        "input" => !matches!(
            attr(attrs, "type").map(str::to_ascii_lowercase).as_deref(),
            Some("hidden" | "submit" | "reset" | "button" | "image")
        ),
        "select" | "textarea" | "meter" | "output" | "progress" => true,
        _ => false,
    }
}

rule! {
    InputMissingLabel {
        id: "input-missing-label",
        category: Correctness,
        default: Off,
        description: "An `<input>` (not hidden or a button), `<select>` or `<textarea>` that no \
                  `<label>` names: it is not inside one, no `<label for>` points at its `id`, \
                  and it has no `aria-label` or `aria-labelledby`.",
        example: "<input type=\"text\">   <label>Name <input type=\"text\"></label>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let tags = tags(file);
        let targets: HashSet<_> = tags
            .iter()
            .filter(|t| t.1 == "label" && t.0.parent().is_some_and(|e| has_text(&e)))
            .filter_map(|t| attr(&t.3, "for"))
            .collect();
        for (tag, name, range, attrs) in &tags {
            if !matches!(name.as_str(), "input" | "select" | "textarea") || !labelable(name, attrs) {
                continue;
            }
            let named = attr(attrs, "aria-label").is_some()
                || attr(attrs, "aria-labelledby").is_some()
                || attr(attrs, "id").is_some_and(|id| targets.contains(id))
                || tag.ancestors().skip(2).filter(|a| a.kind() == k::ELEMENT).any(|e| {
                    e.child(k::START_TAG)
                        .and_then(|t| dom::tag_name(&t, file.text))
                        .is_some_and(|(n, _)| n == "label")
                        && has_text(&e)
                });
            if !named {
                ctx.report(*range, format!("`<{name}>` has no label"));
            }
        }
    }
}

rule! {
    MultipleLabeledControls {
        id: "multiple-labeled-controls",
        category: Correctness,
        default: Off,
        description: "A `<label>` without `for` that holds more than one control: it labels only \
                  the first.",
        example: "<label>A <input> <input></label>   <label>A <input></label>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for (tag, name, range, attrs) in tags(file) {
            if name != "label" || attr(&attrs, "for").is_some() {
                continue;
            }
            let Some(element) = tag.parent() else {
                continue;
            };
            let controls = dom::start_tags(&element)
                .skip(1)
                .filter(|t| {
                    dom::tag_name(t, file.text)
                        .is_some_and(|(n, _)| n == "button" || labelable(&n, &dom::attrs(t, file.text)))
                })
                .count();
            if controls > 1 {
                ctx.report(range, "the label holds more than one control");
            }
        }
    }
}

rule! {
    UniqueLandmark {
        id: "unique-landmark",
        category: Correctness,
        default: Off,
        description: "Landmarks of one kind (`<nav>`, `<aside>`, or an element with the same \
                  landmark `role`) that a screen reader user cannot tell apart: with several of \
                  them, each needs an `aria-label` or `aria-labelledby` that no other has.",
        example: "<nav></nav><nav></nav>   <nav aria-label=\"Main\"></nav><nav aria-label=\"Footer\"></nav>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut groups: std::collections::BTreeMap<String, Vec<(&str, TextRange)>> = Default::default();
        for (_, name, range, attrs) in tags(file) {
            let role = match (attr(&attrs, "role"), name.as_str()) {
                (Some(r), _) => r.trim().to_ascii_lowercase(),
                (None, "nav") => "navigation".into(),
                (None, "aside") => "complementary".into(),
                (None, "main") => "main".into(),
                _ => continue,
            };
            if matches!(
                role.as_str(),
                "navigation" | "complementary" | "main" | "banner" | "contentinfo" | "region" | "search" | "form"
            ) {
                let label = attr(&attrs, "aria-label").or_else(|| attr(&attrs, "aria-labelledby"));
                groups.entry(role).or_default().push((label.unwrap_or("").trim(), range));
            }
        }
        for landmarks in groups.values().filter(|g| g.len() > 1) {
            for (label, range) in landmarks {
                if label.is_empty() || landmarks.iter().filter(|(l, _)| l == label).count() > 1 {
                    ctx.report(*range, "landmarks of one kind need a unique, non-empty accessible name");
                }
            }
        }
    }
}

rule! {
    MapDupName {
        id: "map-dup-name",
        category: Correctness,
        default: Warn,
        description: "A `<map>` whose `name` an earlier `<map>` already has: `usemap` finds the first.",
        example: "<map name=\"a\"></map><map name=\"a\"></map>   // reported at the second",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut seen = Repeats::default();
        for (_, name, _, attrs) in tags(file) {
            for a in attrs.iter().filter(|a| name == "map" && a.name == "name") {
                if let (Some(v), Some(r)) = (a.value, a.value_range) {
                    seen.check(ctx, v, r, || format!("duplicate map name `{v}`"), "first used here");
                }
            }
        }
    }
}

rule! {
    NoTrailingWhitespace {
        id: "no-trailing-whitespace",
        category: Style,
        default: Off,
        description: "Spaces or tabs at the end of a line.",
        example: "<p>text   \n</p>   // reported at the spaces",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut start = 0;
        for line in file.text.split('\n') {
            let body = line.trim_end_matches('\r');
            let kept = body.trim_end_matches([' ', '\t']).len();
            if kept < body.len() {
                let at = TextSize::try_from(start + kept).unwrap_or_default();
                let len = TextSize::try_from(body.len() - kept).unwrap_or_default();
                ctx.report(TextRange::at(at, len), "trailing whitespace");
            }
            start += line.len() + 1;
        }
    }
}

rule! {
    UnrecognizedCharRef {
        id: "unrecognized-char-ref",
        category: Correctness,
        default: Warn,
        description: "A named character reference in text (`&name;`) that HTML does not define. \
                  Numeric references and references in attribute values are not checked.",
        example: "<p>&foo;</p>   <p>&amp;</p>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for t in file.root.descendants_with_tokens().filter_map(|e| e.into_token()) {
            if t.kind() != k::ENTITY {
                continue;
            }
            let text = &file.text[t.text_range()];
            if let Some(name) = text.strip_prefix('&').and_then(|r| r.strip_suffix(';'))
                && !name.starts_with('#')
                && !meta::has("entities", name)
            {
                ctx.report(t.text_range(), format!("unknown character reference `{text}`"));
            }
        }
    }
}
