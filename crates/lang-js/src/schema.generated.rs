// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const SEMI: &str = "semi";
#[allow(dead_code)]
pub const QUOTE_STYLE: &str = "quote_style";

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
    ])
    .with_presets(vec![
    ])
}
