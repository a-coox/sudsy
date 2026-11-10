// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const COLLAPSE: &str = "collapse";
#[allow(dead_code)]
pub const TRAILING_COMMAS: &str = "trailing_commas";
#[allow(dead_code)]
pub const ALIGN_COMMENTS: &str = "align_comments";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::line_width(80),
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::bracket_spacing(true),
        core_config::shared::array_bracket_spacing(false),
        core_config::OptionDef::choice("collapse", &["preserve", "always", "never"], "preserve", "Arrays on one line: `preserve` keeps one that has a line break between its brackets expanded and puts the others on one line if they fit; `always` puts every one that fits on one line; `never` puts every non-empty one on several lines."),
        core_config::OptionDef::choice("trailing_commas", &["never", "multiline"], "multiline", "Trailing comma after the last item of a multi-line array."),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between entries, tables and comments."),
        core_config::OptionDef::boolean("align_comments", false, "Start the trailing comments of consecutive entries in one column (entries with no              blank line or comment line between them); a run with an entry on several lines is not \n             aligned."),
    ])
    .with_presets(vec![
        core_config::Preset::new("toml/default", include_str!("../presets/default.yaml")),
        core_config::Preset::new("toml/taplo-compat", include_str!("../presets/taplo-compat.yaml")),
    ])
}
