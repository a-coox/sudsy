// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const SEQUENCE_INDENT: &str = "sequence_indent";
#[allow(dead_code)]
pub const QUOTE_STYLE: &str = "quote_style";
#[allow(dead_code)]
pub const DOCUMENT_START: &str = "document_start";
#[allow(dead_code)]
pub const DOCUMENT_END: &str = "document_end";
#[allow(dead_code)]
pub const BLANK_LINES_AFTER_KEY: &str = "blank_lines_after_key";
#[allow(dead_code)]
pub const COMMENT_SPACING: &str = "comment_spacing";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::line_width(80),
        core_config::shared::indent_size(2),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::bracket_spacing(true),
        core_config::shared::array_bracket_spacing(false),
        core_config::OptionDef::choice("sequence_indent", &["preserve", "indented", "flush"], "preserve", "A block sequence that is the value of a mapping key: `indented` puts the dashes one level in, `flush` at the key's column, `preserve` keeps each sequence as written."),
        core_config::OptionDef::choice("quote_style", &["preserve", "single", "double", "prefer_double"], "preserve", "Quote style of quoted scalars. With `single` and `double` a scalar keeps its quotes when the other style would change its value or need more escapes; `prefer_double` uses double quotes, except single quotes for text that contains a `\"` (unless it has other escapes, which keep it as written)."),
        core_config::OptionDef::choice("document_start", &["preserve", "always", "never"], "preserve", "The `---` marker: `always` starts every document with it, `never` leaves it out of the first document when that is possible, `preserve` keeps what is written."),
        core_config::OptionDef::choice("document_end", &["preserve", "always", "never"], "preserve", "The `...` marker: `always` ends every document with it, `never` leaves it out unless the next document has a directive or no `---`, or the document is empty, `preserve` keeps what is written."),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between entries, comments and documents."),
        core_config::OptionDef::choice("blank_lines_after_key", &["preserve", "remove"], "preserve", "Blank lines between a key and its block value on the next line: `preserve` keeps \n             them (up to `max_blank_lines`); `remove` drops them."),
        core_config::OptionDef::int("comment_spacing", 1, (1, 10), "Spaces between a value and a comment after it on the same line."),
    ])
    .with_presets(vec![
        core_config::Preset::new("yaml/default", include_str!("../presets/default.yaml")),
        core_config::Preset::new("yaml/prettier-compat", include_str!("../presets/prettier-compat.yaml")),
        core_config::Preset::new("yaml/yamllint", include_str!("../presets/yamllint.yaml")),
    ])
}
