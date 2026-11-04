//! Rules about the wording of `<summary>`: property summaries (SA1623, SA1624) and the
//! standard text of constructors and finalizers (SA1642, SA1643). Fixes change documentation
//! text, so they are unsafe.

use super::doc_comments::{
    Access, DOC_KINDS, Decl, Documented, Element, documented, first_text, has_modifier,
    modifier_access, normalized_text, text_tokens,
};
use super::kind;
use core_rules::rule;
use core_rules::tree::significant_range;
use core_rules::{Edit, Fix, FixKind, RuleContext};
use core_syntax::{SyntaxNode, TextRange};

/// What is wrong with a property summary.
struct Mismatch {
    /// SA1624: the summary names an accessor the caller cannot reach (`get`/`set`).
    restricted: Option<&'static str>,
    expected: &'static str,
    /// The wrong start of the text, to take out.
    remove: Option<String>,
}

struct Starts {
    gets: &'static str,
    sets: &'static str,
    gets_or_sets: &'static str,
    returns: &'static str,
}

const PLAIN: Starts = Starts {
    gets: "Gets",
    sets: "Sets",
    gets_or_sets: "Gets or sets",
    returns: "Returns",
};

const WHETHER: Starts = Starts {
    gets: "Gets a value indicating whether",
    sets: "Sets a value indicating whether",
    gets_or_sets: "Gets or sets a value indicating whether",
    returns: "Returns a value indicating whether",
};

fn starts_with_ci(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|t| t.eq_ignore_ascii_case(prefix))
}

fn is_bool(ty: &SyntaxNode, grammar: &core_syntax::Grammar) -> bool {
    let ty = if ty.kind() == kind::REF_TYPE {
        ty.children()
            .find(|c| grammar.has_tag(c.kind(), "type"))
            .unwrap_or_else(|| ty.clone())
    } else {
        ty.clone()
    };
    let text: String = significant_text(&ty, grammar);
    matches!(
        text.as_str(),
        "bool" | "Boolean" | "System.Boolean" | "global::System.Boolean"
    )
}

fn significant_text(node: &SyntaxNode, grammar: &core_syntax::Grammar) -> String {
    let range = significant_range(node, grammar);
    let full = node.text().to_string();
    let start = usize::from(range.start() - node.text_range().start());
    let end = usize::from(range.end() - node.text_range().start());
    full[start..end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// StyleCop's `PropertySummaryDocumentationAnalyzer`: which accessors a reader can reach decides
/// how the summary starts.
fn property_mismatch(d: &Documented, ctx: &RuleContext<'_>) -> Option<(Mismatch, Element)> {
    if d.doc.has_inheritdoc() {
        return None;
    }
    let summary = d.doc.first("summary").filter(|s| !s.is_empty_element())?;
    let grammar = ctx.file.grammar;
    let starts = if d.return_type(grammar).is_some_and(|t| is_bool(&t, grammar)) {
        &WHETHER
    } else {
        &PLAIN
    };
    let (getter, setter) = d.accessors();
    let expression_bodied = d
        .node
        .children()
        .any(|c| c.kind() == kind::ARROW_EXPRESSION_CLAUSE);
    let text = first_text(&summary.node)
        .map(|t| normalized_text(&t).trim_start().to_string())
        .unwrap_or_default();
    let gets_or_sets = starts_with_ci(&text, starts.gets_or_sets);
    let gets = !gets_or_sets && starts_with_ci(&text, starts.gets);
    let sets = starts_with_ci(&text, starts.sets);

    let (get_visible, set_visible) = match (&getter, &setter) {
        (Some(g), Some(s)) => {
            let plain = |a: &SyntaxNode| modifier_access(a).is_none();
            if plain(g) && plain(s) {
                (true, true)
            } else if has_modifier(g, kind::PRIVATE_KEYWORD) {
                (false, true)
            } else if has_modifier(s, kind::PRIVATE_KEYWORD) {
                (true, false)
            } else if d.effective_access() <= Access::Internal
                && d.effective_access() != Access::Protected
            {
                // Only reachable inside the assembly: both count.
                (true, true)
            } else {
                let exposed = |a: &SyntaxNode| {
                    matches!(
                        d.accessor_access(a),
                        Access::Public | Access::ProtectedInternal | Access::Protected
                    )
                };
                (exposed(g), exposed(s))
            }
        }
        _ => (
            getter.is_some() || expression_bodied,
            getter.is_none() && !expression_bodied && setter.is_some(),
        ),
    };
    let wrong = |expected, remove: &[&str]| {
        let found = remove
            .iter()
            .find(|r| starts_with_ci(&text, r))
            .map(|r| text[..r.len()].to_string());
        Mismatch {
            restricted: None,
            expected,
            remove: found,
        }
    };
    let restricted = |accessor, expected, remove: &'static str| Mismatch {
        restricted: Some(accessor),
        expected,
        remove: Some(remove.to_string()),
    };
    let mismatch = if get_visible && set_visible {
        (!gets_or_sets).then(|| {
            wrong(
                starts.gets_or_sets,
                &[starts.gets, starts.sets, starts.returns],
            )
        })
    } else if get_visible {
        if setter.is_some() {
            (!gets).then(|| {
                if gets_or_sets {
                    restricted("get", starts.gets, starts.gets_or_sets)
                } else {
                    wrong(starts.gets, &[starts.sets, starts.returns])
                }
            })
        } else {
            (!gets).then(|| {
                wrong(
                    starts.gets,
                    &[starts.sets, starts.gets_or_sets, starts.returns],
                )
            })
        }
    } else if set_visible {
        if getter.is_some() {
            (!sets).then(|| {
                if gets_or_sets {
                    restricted("set", starts.sets, starts.gets_or_sets)
                } else {
                    wrong(starts.sets, &[starts.gets, starts.returns])
                }
            })
        } else {
            (!sets).then(|| {
                wrong(
                    starts.sets,
                    &[starts.gets_or_sets, starts.gets, starts.returns],
                )
            })
        }
    } else {
        None
    };
    mismatch.map(|m| (m, summary.clone()))
}

/// The fix that gives the summary the expected start (StyleCop's `PropertySummaryDocumentation` fix).
fn property_fix(m: &Mismatch, summary: &Element) -> Option<Fix> {
    if summary.content().is_empty() {
        return None;
    }
    let text_node = first_text(&summary.node)?;
    let token = text_tokens(&text_node)
        .into_iter()
        .find(|t| t.kind() == kind::XML_TEXT_LITERAL_TOKEN)?;
    let text = token.text();
    let lead = text.len() - text.trim_start().len();
    let rest = match m
        .remove
        .as_deref()
        .and_then(|r| text.find(r).map(|i| i + r.len()))
    {
        Some(after) => text[after..].trim_start(),
        None => &text[lead..],
    };
    let mut chars = rest.chars();
    let lowered: String = chars
        .next()
        .map(|c| c.to_lowercase().chain(chars).collect())
        .unwrap_or_default();
    let new = format!("{}{} {}", &text[..lead], m.expected, lowered);
    Some(Fix::new(
        format!("Start the summary with \"{}\"", m.expected),
        FixKind::Unsafe,
        vec![Edit::replace(token.text_range(), new)],
    ))
}

fn report_property(rule_restricted: bool, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(d) = documented(node, ctx, |k| k == Decl::Property) else {
        return;
    };
    let Some((m, summary)) = property_mismatch(&d, ctx) else {
        return;
    };
    if m.restricted.is_some() != rule_restricted {
        return;
    }
    let message = match m.restricted {
        Some(accessor) => format!(
            "the property only has a visible {accessor} accessor: the summary should start with '{}'",
            m.expected
        ),
        None => format!("the summary should start with '{}'", m.expected),
    };
    let fix = property_fix(&m, &summary);
    for range in d.names() {
        let diagnostic = ctx.report(range, message.clone());
        if let Some(fix) = fix.clone() {
            diagnostic.fix(fix);
        }
    }
}

rule! {
    pub PropertySummaryMatchesAccessors {
        id: "property-summary-matches-accessors",
        category: Style,
        default: Off,
        fix: Unsafe,
        description: "A property summary starts with \"Gets\", \"Sets\" or \"Gets or sets\" (\"Gets a value \
                  indicating whether\" and so on for `bool`), as the property's accessors reachable from outside \
                  the type say. The type is read from the declaration (`bool`, `Boolean`, `System.Boolean`). \
                  The fix replaces the start of the text and lower-cases what follows.",
        example: "/// <summary>The count.</summary>\npublic int Count { get; set; }   // Gets or sets the count.",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        report_property(false, node, ctx);
    }
}

rule! {
    pub PropertySummaryOmitsRestrictedAccessor {
        id: "property-summary-omits-restricted-accessor",
        category: Style,
        default: Off,
        fix: Unsafe,
        description: "A property summary says \"Gets or sets\" while one of the accessors is not reachable \
                  from outside the type (`private set`, `internal get` on a public property). The fix \
                  replaces the start of the text with the one that matches the visible accessor.",
        example: "/// <summary>Gets or sets the count.</summary>\npublic int Count { get; private set; }   // Gets the count.",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        report_property(true, node, ctx);
    }
}

/// Where the standard text of a constructor or finalizer is checked.
enum Found {
    Match,
    /// The text is wrong; fix by inserting the standard text before the content.
    Text,
    /// The text is right, the `<see>` is not.
    See(TextRange),
}

fn type_of(d: &Documented) -> Option<(SyntaxNode, String, Option<String>)> {
    let parent = d.node.parent()?;
    let name = parent
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind::IDENTIFIER_TOKEN)?
        .text()
        .to_string();
    let generics = parent
        .children()
        .find(|c| c.kind() == kind::TYPE_PARAMETER_LIST)
        .map(|l| {
            l.children()
                .filter_map(|p| {
                    p.children_with_tokens()
                        .filter_map(|e| e.into_token())
                        .find(|t| t.kind() == kind::IDENTIFIER_TOKEN)
                })
                .map(|t| t.text().to_string())
                .collect::<Vec<_>>()
                .join(",")
        });
    Some((parent, name, generics))
}

/// Does the summary start with `first`, `<see cref="Type"/>`, `second`? `None` without a
/// `<summary>` element (nothing to check).
fn standard_text(d: &Documented, name: &str, first: &str, second: &str) -> Option<Found> {
    let summary = d.doc.first("summary").filter(|s| !s.is_empty_element())?;
    let content = summary.content();
    if let [text, see, rest, ..] = content.as_slice()
        && text.kind() == kind::XML_TEXT
        && see.kind() == kind::XML_EMPTY_ELEMENT
        && rest.kind() == kind::XML_TEXT
        && normalized_text(text).trim_start() == first
        && normalized_text(rest).starts_with(second)
    {
        let see = Element::new(see.clone());
        let cref = see
            .attr("cref")
            .filter(|a| a.kind == kind::XML_CREF_ATTRIBUTE);
        let right = cref.is_some_and(|a| {
            let base = a.value.split(['{', '<']).next().unwrap_or_default();
            base.rsplit('.').next() == Some(name)
        });
        return Some(if right {
            Found::Match
        } else {
            Found::See(see.range())
        });
    }
    Some(Found::Text)
}

fn see_element(name: &str, generics: Option<&str>) -> String {
    match generics {
        Some(g) => format!("<see cref=\"{name}{{{g}}}\"/>"),
        None => format!("<see cref=\"{name}\"/>"),
    }
}

/// A standard sentence with the wrong ending (`... <see cref="T"/> struct.` for a class): the
/// part after the `first` text is replaced, not repeated.
fn malformed_sentence(summary: &Element, see: &str, first: &str, second: &str) -> Option<Edit> {
    let content = summary.content();
    let [text, old_see, rest, ..] = content.as_slice() else {
        return None;
    };
    if text.kind() != kind::XML_TEXT
        || old_see.kind() != kind::XML_EMPTY_ELEMENT
        || rest.kind() != kind::XML_TEXT
        || normalized_text(text).trim_start() != first
    {
        return None;
    }
    let token = text_tokens(rest).into_iter().next()?;
    let raw = token.text();
    let body = raw.trim_start();
    let lead = raw.len() - body.len();
    let word = body.split(' ').next().unwrap_or_default();
    if lead == 0 || word.is_empty() {
        return None;
    }
    let end = token.text_range().start() + core_syntax::TextSize::from((lead + word.len()) as u32);
    let second = second.trim_end();
    let period = if word.ends_with('.') && !second.ends_with('.') {
        "."
    } else {
        ""
    };
    Some(Edit::replace(
        TextRange::new(old_see.text_range().start(), end),
        format!("{see}{second}{period}"),
    ))
}

fn standard_text_fix(
    d: &Documented,
    found: &Found,
    name: &str,
    generics: Option<&str>,
    first: &str,
    second: &str,
) -> Option<Fix> {
    let summary = d.doc.first("summary")?;
    let see = see_element(name, generics);
    let edit = match found {
        Found::Match => return None,
        Found::See(range) => Edit::replace(*range, see),
        Found::Text => {
            if let Some(edit) = malformed_sentence(summary, &see, first, second) {
                return Some(Fix::new(
                    "Use the standard summary text",
                    FixKind::Unsafe,
                    vec![edit],
                ));
            }
            let tag = summary
                .node
                .children()
                .find(|c| c.kind() == kind::XML_ELEMENT_START_TAG)?;
            let at = tag.text_range().end();
            let second = second.trim_end();
            let sentence = format!(
                "{first}{see}{second}{}",
                if second.ends_with('.') { "" } else { "." }
            );
            let starts_with_space = summary
                .content()
                .first()
                .is_some_and(|c| c.text().to_string().starts_with(char::is_whitespace));
            Edit::insert(
                at,
                if starts_with_space {
                    sentence
                } else {
                    format!("{sentence} ")
                },
            )
        }
    };
    Some(Fix::new(
        "Use the standard summary text",
        FixKind::Unsafe,
        vec![edit],
    ))
}

fn check_standard(
    d: &Documented,
    ctx: &mut RuleContext<'_>,
    parts: &[(&str, &str)],
    report_last_only: bool,
) {
    let Some((_, name, generics)) = type_of(d) else {
        return;
    };
    let Some(summary) = d.doc.first("summary") else {
        return;
    };
    let mut result = None;
    for (i, (first, second)) in parts.iter().enumerate() {
        let found = standard_text(d, &name, first, second);
        match found {
            Some(Found::Match) | None => return,
            Some(f) => {
                if !report_last_only || i + 1 == parts.len() {
                    result = Some((f, *first, *second));
                }
            }
        }
    }
    let Some((found, first, second)) = result else {
        return;
    };
    let range = match &found {
        Found::See(r) => *r,
        _ => summary.range(),
    };
    let fix = standard_text_fix(d, &found, &name, generics.as_deref(), first, second);
    let diagnostic = ctx.report(
        range,
        format!(
            "the summary should start with: \"{first}<see cref=\"{name}\"/>{}\"",
            second.trim_end()
        ),
    );
    if let Some(fix) = fix {
        diagnostic.fix(fix);
    }
}

rule! {
    pub ConstructorSummaryStandardText {
        id: "constructor-summary-standard-text",
        category: Style,
        default: Off,
        fix: Unsafe,
        description: "The summary of a constructor starts with \"Initializes a new instance of the <see cref=\"T\"/> \
                  class\" (struct), \"Initializes static members of the ...\" for a static constructor, and a \
                  private constructor may say \"Prevents a default instance of the ... from being created\". The \
                  `<see>` is compared with the name of the type. The fix puts the standard sentence before the \
                  text.",
        example: "/// <summary>Creates it.</summary>\npublic Foo() { }   // Initializes a new instance of the <see cref=\"Foo\"/> class.",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, |k| k == Decl::Constructor) else {
            return;
        };
        let is_struct = d
            .node
            .parent()
            .is_some_and(|p| matches!(p.kind(), kind::STRUCT_DECLARATION | kind::RECORD_STRUCT_DECLARATION));
        let word = if is_struct { "struct" } else { "class" };
        let non_private = (
            "Initializes a new instance of the ",
            &format!(" {word}"),
        );
        if has_modifier(&d.node, kind::STATIC_KEYWORD) {
            check_standard(
                &d,
                ctx,
                &[("Initializes static members of the ", &format!(" {word}"))],
                false,
            );
        } else if has_modifier(&d.node, kind::PRIVATE_KEYWORD) {
            check_standard(
                &d,
                ctx,
                &[
                    (
                        "Prevents a default instance of the ",
                        &format!(" {word} from being created"),
                    ),
                    (non_private.0, non_private.1),
                ],
                true,
            );
        } else {
            check_standard(&d, ctx, &[(non_private.0, non_private.1)], false);
        }
    }
}

rule! {
    pub DestructorSummaryStandardText {
        id: "destructor-summary-standard-text",
        category: Style,
        default: Off,
        fix: Unsafe,
        description: "The summary of a finalizer starts with \"Finalizes an instance of the <see cref=\"T\"/> \
                  class.\". The `<see>` is compared with the name of the type. The fix puts the standard \
                  sentence before the text.",
        example: "/// <summary>Cleans up.</summary>\n~Foo() { }   // Finalizes an instance of the <see cref=\"Foo\"/> class.",
        kinds: DOC_KINDS,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(d) = documented(node, ctx, |k| k == Decl::Destructor) else {
            return;
        };
        check_standard(
            &d,
            ctx,
            &[("Finalizes an instance of the ", " class.")],
            false,
        );
    }
}
