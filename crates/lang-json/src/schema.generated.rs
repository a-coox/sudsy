// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const COLLAPSE: &str = "collapse";
#[allow(dead_code)]
pub const TRAILING_COMMAS: &str = "trailing_commas";
#[allow(dead_code)]
pub const DIALECT: &str = "dialect";

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
        core_config::OptionDef::choice("collapse", &["preserve", "always", "never", "opening_break"], "preserve", "Objects and arrays on one line: `preserve` keeps one that has a line break between its brackets expanded and puts the others on one line if they fit; `always` puts every one that fits on one line; `never` puts every non-empty one on several lines; `opening_break` keeps an object expanded when a line break follows its opening brace and puts every array that fits on one line."),
        core_config::OptionDef::choice("trailing_commas", &["never", "multiline"], "never", "Trailing commas after the last item of a multi-line object or array (JSONC only)."),
        core_config::OptionDef::choice("dialect", &["auto", "json", "jsonc"], "auto", "JSON or JSONC (comments, trailing commas); `auto` decides by file name."),
    ])
    .with_presets(vec![
        core_config::Preset::new("json/biome", include_str!("../presets/biome.yaml")),
        core_config::Preset::new("json/prettier-compat", include_str!("../presets/prettier-compat.yaml")),
    ])
}
