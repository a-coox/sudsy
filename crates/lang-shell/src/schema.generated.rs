// Generated from lang.yaml by `cargo xtask codegen`.

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(4),
        core_config::OptionDef::choice("indent_style", &["space", "tab"], "tab", "Indent with spaces or tabs."),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between statements and comments."),
    ])
    .with_presets(vec![
        core_config::Preset::new("shell/shellcheck", include_str!("../presets/shellcheck.yaml")),
    ])
}
