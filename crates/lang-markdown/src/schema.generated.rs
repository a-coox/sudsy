// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const EMBEDDED_LANGUAGE_FORMATTING: &str = "embedded_language_formatting";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(2),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::choice("embedded_language_formatting", &["auto", "off"], "auto", "Format the code of a fenced block with the formatter of the language its info string names (`auto`, Prettier's `embeddedLanguageFormatting`), or keep it as written (`off`). Code that does not parse, or of a language that is not installed, is kept either way."),
    ])
    .with_presets(vec![
        core_config::Preset::new("markdown/prettier-compat", include_str!("../presets/prettier-compat.yaml")),
    ])
}
