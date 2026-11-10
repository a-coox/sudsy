//! `quoted-strings`: which strings must, may or must not be quoted, and with which quotes.

use crate::format::{Quotes, restyle};
use crate::generated::{Scalar, kind};
use crate::resolve::{plain_round_trips, resolves_to_str};
use crate::rules::is_key;
use crate::scalar::{double_quoted_value, plain_value, single_quoted_value};
use core_rules::rule;
use core_rules::sorting::cached_regex;
use core_rules::{Check, Edit, FileContext, Fix, FixKind, Opt, OptionDecl, RuleContext};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxNodeExt, SyntaxToken};
use regex::Regex;

/// What is wrong with a scalar.
enum Problem {
    Missing,
    WrongType,
    Redundant,
}

struct Settings {
    quote_type: String,
    required: String,
    extra_required: Vec<Regex>,
    extra_allowed: Vec<Regex>,
    allow_quoted_quotes: bool,
}

impl Settings {
    fn matches(regexes: &[Regex], value: &str) -> bool {
        regexes.iter().any(|r| r.is_match(value))
    }
}

/// The quote character of a quoted scalar.
fn style_of(kind: SyntaxKind) -> Option<char> {
    match kind {
        kind::SINGLE_QUOTED => Some('\''),
        kind::DOUBLE_QUOTED => Some('"'),
        _ => None,
    }
}

/// Do `text` (a double-quoted token) and `value` need the quotes to be read back as they are?
fn quotes_are_needed(token: &SyntaxToken, value: &str, in_flow: bool) -> bool {
    if in_flow && value.contains([',', '[', ']', '{', '}']) {
        return true;
    }
    if token.kind() == kind::DOUBLE_QUOTED {
        let text = token.text();
        // Escaped line breaks and special characters stay escaped.
        if text.contains("\\\n") || text.contains("\\\r\n") {
            return true;
        }
    }
    !plain_round_trips(value)
}

/// `value` as a quoted scalar of the type wanted, or of the type with fewer escapes.
fn quote(value: &str, wanted: Option<char>) -> String {
    let double = wanted.map_or(!value.contains(['"', '\\']), |c| c == '"');
    if double {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

rule! {
    pub QuotedStrings {
        id: "quoted-strings",
        category: Style,
        default: Off,
        fix: Unsafe,
        description: "Which strings are quoted. Plain scalars that are not strings (`12`, `true`, \
                  `null`, dates, ...), block scalars and scalars tagged `!!` are never checked. \
                  `required: always` wants quotes on every string, `only_when_needed` reports \
                  quotes that change nothing (unless `extra_allowed` matches) and plain strings \
                  that `extra_required` matches, `optional` only the latter. Fixes: quoting a \
                  plain string and switching the quote type keep the value; removing quotes is \
                  unsafe because YAML 1.2 parsers read some strings (`1e3`) as numbers.",
        example: "name: bar   # with required: always; becomes name: \"bar\"",
        options: [
            OptionDecl {
                name: "quote_type",
                default: Opt::Str("any"),
                check: Check::OneOf(&["any", "single", "double", "consistent"]),
                description: "The quotes to use. `consistent`: the first quoted string of the file \
                          decides.",
            },
            OptionDecl {
                name: "required",
                default: Opt::Str("always"),
                check: Check::OneOf(&["always", "optional", "only_when_needed"]),
                description: "`always`: every string is quoted. `optional`: quotes are free, but \
                          must be of `quote_type`. `only_when_needed`: quotes only where the \
                          string would otherwise read as something else.",
            },
            OptionDecl {
                name: "extra_required",
                default: Opt::List(&[]),
                check: Check::Regex,
                description: "Regular expressions; a string they match (searched in the value) must \
                          be quoted even with `optional` or `only_when_needed`.",
            },
            OptionDecl {
                name: "extra_allowed",
                default: Opt::List(&[]),
                check: Check::Regex,
                description: "Regular expressions; a string they match may be quoted with \
                          `only_when_needed`.",
            },
            OptionDecl {
                name: "allow_quoted_quotes",
                default: Opt::Bool(false),
                check: Check::Any,
                description: "Accept the other quote type when the string contains the wanted \
                          quote.",
            },
            OptionDecl {
                name: "check_keys",
                default: Opt::Bool(false),
                check: Check::Any,
                description: "Check mapping keys too.",
            },
    ],
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let o = ctx.options;
        let compile = |name: &str| -> Vec<Regex> {
            o.list(name).into_iter().filter_map(cached_regex).collect()
        };
        let settings = Settings {
            quote_type: o.string("quote_type").to_string(),
            required: o.string("required").to_string(),
            extra_required: compile("extra_required"),
            extra_allowed: compile("extra_allowed"),
            allow_quoted_quotes: o.bool("allow_quoted_quotes"),
        };
        let check_keys = o.bool("check_keys");
        // The style of the first quoted string, for `consistent`.
        let mut consistent: Option<char> = None;
        for node in file.root.descendants().filter(|n| n.kind() == kind::SCALAR) {
            let Some(token) = Scalar::cast(node.clone()).and_then(|s| s.value()) else {
                continue;
            };
            let key = is_key(&node);
            if key && !check_keys || has_double_bang_tag(&node) {
                continue;
            }
            let text = token.text();
            let Some(value) = (match token.kind() {
                kind::PLAIN => Some(plain_value(text)),
                kind::SINGLE_QUOTED => Some(single_quoted_value(text)),
                _ => double_quoted_value(text),
            }) else {
                continue;
            };
            let style = style_of(token.kind());
            let is_str = !value.is_empty() && resolves_to_str(&value);
            if style.is_none() && !is_str {
                continue;
            }
            let in_flow = node
                .ancestors()
                .any(|a| matches!(a.kind(), kind::FLOW_MAPPING | kind::FLOW_SEQUENCE));
            let quoted_quotes = (style == Some('\'') && value.contains('"'))
                || (style == Some('"') && value.contains('\''));
            let mut type_matches = |style: char| -> bool {
                match settings.quote_type.as_str() {
                    "consistent" => *consistent.get_or_insert(style) == style,
                    "any" => true,
                    "single" => style == '\'',
                    _ => style == '"',
                }
            };
            let problem = match (settings.required.as_str(), style) {
                ("always", None) => Some(Problem::Missing),
                ("always", Some(s)) => (!(type_matches(s)
                    || settings.allow_quoted_quotes && quoted_quotes))
                    .then_some(Problem::WrongType),
                ("optional", Some(s)) => (!type_matches(s)
                    && !(settings.allow_quoted_quotes && quoted_quotes))
                    .then_some(Problem::WrongType),
                ("only_when_needed", Some(s)) => {
                    if is_str && !quotes_are_needed(&token, &value, in_flow) {
                        let extra = Settings::matches(&settings.extra_required, &value)
                            || Settings::matches(&settings.extra_allowed, &value);
                        (!extra).then_some(Problem::Redundant)
                    } else {
                        (!type_matches(s) && !(settings.allow_quoted_quotes && quoted_quotes))
                            .then_some(Problem::WrongType)
                    }
                }
                (_, None) => {
                    Settings::matches(&settings.extra_required, &value).then_some(Problem::Missing)
                }
                _ => None,
            };
            let Some(problem) = problem else {
                continue;
            };
            let what = if key { "key" } else { "value" };
            let quote_type = &settings.quote_type;
            let wanted = match quote_type.as_str() {
                "single" => Some('\''),
                "double" => Some('"'),
                "consistent" => consistent,
                _ => None,
            };
            let range = token.text_range();
            let one_line = !text.contains(['\n', '\r']);
            let report = match problem {
                Problem::Missing => {
                    let message = if quote_type == "any" || settings.required != "always" {
                        format!("string {what} is not quoted")
                    } else {
                        format!("string {what} is not quoted with {quote_type} quotes")
                    };
                    let report = ctx.report(range, message);
                    if one_line {
                        report.fix(Fix::new(
                            "Quote the string",
                            FixKind::Safe,
                            vec![Edit::replace(range, quote(&value, wanted))],
                        ));
                    }
                    continue;
                }
                Problem::WrongType => {
                    format!("string {what} is not quoted with {quote_type} quotes")
                }
                Problem::Redundant => {
                    let report = ctx.report(
                        range,
                        format!("string {what} is redundantly quoted with {quote_type} quotes"),
                    );
                    if one_line {
                        report.fix(Fix::new(
                            "Remove the quotes",
                            FixKind::Unsafe,
                            vec![Edit::replace(range, value)],
                        ));
                    }
                    continue;
                }
            };
            let report_ = ctx.report(range, report);
            let target = match wanted {
                Some('\'') => Some(Quotes::Single),
                Some(_) => Some(Quotes::Double),
                None => None,
            };
            if let Some(new) = target.and_then(|q| restyle(text, token.kind(), q)) {
                report_.fix(Fix::new(
                    "Change the quote type",
                    FixKind::Safe,
                    vec![Edit::replace(range, new)],
                ));
            }
        }
    }
}

/// Does the scalar carry a `!!` tag (`!!str x`, `!!int 42`)? It says what it means.
fn has_double_bang_tag(node: &SyntaxNode) -> bool {
    node.parent().is_some_and(|p| {
        p.kind() == kind::PROPERTY_NODE
            && p.tokens()
                .any(|t| t.kind() == kind::TAG && t.text().starts_with("!!"))
    })
}
