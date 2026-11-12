// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const WHITESPACE_SENSITIVITY: &str = "whitespace_sensitivity";
#[allow(dead_code)]
pub const BRACKET_SAME_LINE: &str = "bracket_same_line";
#[allow(dead_code)]
pub const SINGLE_ATTRIBUTE_PER_LINE: &str = "single_attribute_per_line";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::line_width(80),
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::choice("whitespace_sensitivity", &["css", "strict", "ignore"], "css", "Whether whitespace between inline elements and text is significant: `css` follows each element's CSS display, `strict` treats every element as inline, `ignore` as block."),
        core_config::OptionDef::boolean("bracket_same_line", false, "Put the `>` of a multi-line start tag at the end of the last attribute's line."),
        core_config::OptionDef::boolean("single_attribute_per_line", false, "Put every attribute of a tag that has several on its own line."),
    ])
    .with_presets(vec![
    ])
}
