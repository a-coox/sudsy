// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const SEMI: &str = "semi";
#[allow(dead_code)]
pub const QUOTE_STYLE: &str = "quote_style";
#[allow(dead_code)]
pub const QUOTE_PROPS: &str = "quote_props";
#[allow(dead_code)]
pub const TRAILING_COMMAS: &str = "trailing_commas";
#[allow(dead_code)]
pub const ARROW_PARENS: &str = "arrow_parens";
#[allow(dead_code)]
pub const BRACKET_SAME_LINE: &str = "bracket_same_line";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::line_width(80),
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::boolean("semi", true, "End statements with `;`. When false a statement that starts with `(`, `[`, a backtick, `+`, `-`, `/` or `<` gets a `;` in front, as a class property does before a member that starts with `[`, `(` or `*`."),
        core_config::OptionDef::choice("quote_style", &["double", "single"], "double", "Quotes of strings (not JSX attributes). A string with more of the preferred quote than of the other keeps the other one, to need fewer escapes."),
        core_config::OptionDef::choice("quote_props", &["as_needed", "consistent", "preserve"], "as_needed", "Quotes around property names of objects and destructuring patterns: `as_needed` removes quotes that are not required (`\"a\": 1` becomes `a: 1`), `consistent` quotes every name of an object when one of its names requires quotes and removes them otherwise, `preserve` keeps what the source has. Numeric names are left as written."),
        core_config::OptionDef::choice("trailing_commas", &["all", "es5", "none"], "all", "Trailing comma of a list that breaks over several lines: `all` after every list including function parameters and call arguments, `es5` after objects, arrays, destructuring and import/export lists only, `none` never."),
        core_config::shared::bracket_spacing(true),
        core_config::OptionDef::choice("arrow_parens", &["always", "avoid"], "always", "Parentheses around the lone parameter of an arrow function: `always` writes `(x) => x`, `avoid` writes `x => x` when the parameter is a plain name (no type, default, rest or destructuring)."),
        core_config::OptionDef::boolean("bracket_same_line", false, "JSX: put the `>` of a multi-line element on the line of its last attribute instead of on a line of its own."),
    ])
    .with_presets(vec![
    ])
}
