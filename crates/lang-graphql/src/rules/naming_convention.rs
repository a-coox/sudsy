use crate::generated::kind;
use core_rules::{Check, Opt, OptionDecl, RuleContext, rule};
use core_syntax::{SyntaxNode, SyntaxToken};

const STYLES: [&str; 5] = ["PascalCase", "camelCase", "snake_case", "UPPER_CASE", "off"];

macro_rules! style_option {
    ($name:literal, $default:literal, $what:literal) => {
        OptionDecl {
            name: $name,
            default: Opt::Str($default),
            check: Check::OneOf(&STYLES),
            description: concat!(
                "Style of ",
                $what,
                ": `PascalCase`, `camelCase`, `snake_case`, `UPPER_CASE` or `off`."
            ),
        }
    };
}

fn matches_style(style: &str, name: &str) -> bool {
    let name = name.trim_start_matches('_');
    let first = name.chars().next();
    let plain = name.chars().all(|c| c.is_ascii_alphanumeric());
    match style {
        "PascalCase" => plain && first.is_some_and(|c| c.is_ascii_uppercase()),
        "camelCase" => plain && first.is_some_and(|c| c.is_ascii_lowercase()),
        "snake_case" => !name.chars().any(|c| c.is_ascii_uppercase()),
        "UPPER_CASE" => !name.chars().any(|c| c.is_ascii_lowercase()),
        _ => true,
    }
}

/// The option that styles the name of `node` and the name's token, or `None` for a node that has
/// no checked name.
fn named(node: &SyntaxNode) -> Option<(&'static str, SyntaxToken)> {
    let option = match node.kind() {
        kind::OPERATION => "operations",
        kind::FRAGMENT => "fragments",
        kind::VAR_DEF => "variables",
        kind::FIELD_DEF => "fields",
        kind::ENUM_VALUE_DEF => "enum_values",
        kind::DIRECTIVE_DEF => "directives",
        kind::INPUT_VALUE_DEF => match node.parent()?.kind() {
            kind::ARGS_DEF => "arguments",
            _ => "fields",
        },
        kind::SCALAR_TYPE
        | kind::OBJECT_TYPE
        | kind::INTERFACE_TYPE
        | kind::UNION_TYPE
        | kind::ENUM_TYPE
        | kind::INPUT_TYPE => "types",
        _ => return None,
    };
    let token = super::name_of(node)?;
    Some((option, token))
}

rule! {
    pub NamingConvention {
        id: "naming-convention",
        category: Style,
        default: Warn,
        description: "A name does not follow the style set for what it names: operations, \
                  fragments, variables, fields, arguments, types, enum values and directives. \
                  `PascalCase` and `camelCase` allow letters and digits only, `snake_case` and \
                  `UPPER_CASE` forbid the other case; leading underscores are ignored. No fix: \
                  renaming changes the schema or the queries that use it. Equivalent: \
                  graphql-eslint naming-convention (styles only, no prefix or suffix lists).",
        example: "type user_account {   # reported: PascalCase wanted\n  FirstName: String   # reported: camelCase wanted\n}",
        options: [
            style_option!("operations", "PascalCase", "operation names"),
            style_option!("fragments", "PascalCase", "fragment names"),
            style_option!("variables", "camelCase", "variable names"),
            style_option!("fields", "camelCase", "field and input field names in definitions"),
            style_option!("arguments", "camelCase", "argument names in definitions"),
            style_option!("types", "PascalCase", "type names"),
            style_option!("enum_values", "UPPER_CASE", "enum values"),
            style_option!("directives", "camelCase", "directive names"),
        ],
        kinds: &[kind::DOCUMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        for def in node.descendants() {
            let Some((option, token)) = named(&def) else {
                continue;
            };
            let style = ctx.options.string(option);
            let name = token.text().trim_start_matches('$');
            if !matches_style(style, name) {
                ctx.report(token.text_range(), format!("`{name}` must be {style}"));
            }
        }
    }
}
