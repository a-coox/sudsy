// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const SPACE_AROUND_EQUALS: &str = "space_around_equals";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::boolean("space_around_equals", true, "Write `key = value` rather than `key=value`."),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between settings, sections and comments."),
    ])
    .with_presets(vec![
    ])
}
