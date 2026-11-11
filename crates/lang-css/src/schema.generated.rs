// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const SINGLE_QUOTE: &str = "single_quote";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::line_width(80),
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between statements and comments."),
        core_config::OptionDef::boolean("single_quote", false, "Write strings with single quotes. A string that holds more of the chosen quote than of the other keeps the other."),
    ])
    .with_presets(vec![
        core_config::Preset::new("css/stylelint-recommended", include_str!("../presets/stylelint-recommended.yaml")),
        core_config::Preset::new("css/stylelint-standard", include_str!("../presets/stylelint-standard.yaml")),
    ])
}
