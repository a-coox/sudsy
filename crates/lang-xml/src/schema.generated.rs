// Generated from lang.yaml by `cargo xtask codegen`.

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::line_width(100),
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between elements, comments and declarations."),
    ])
    .with_presets(vec![
        core_config::Preset::new("xml/csharpier-compat", include_str!("../presets/csharpier-compat.yaml")),
    ])
}
