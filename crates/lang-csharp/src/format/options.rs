//! The C# format options (plan.md "Configuration and presets": `brace_newline`,
//! `trailing_commas`, `member_chain_min_calls`, plus the shared ones).

use core_config::{OptionDef, OptionSchema, OptionValue, Preset, ResolvedOptions, shared};
use core_format::shapes::{ListLayout, TrailingComma};

pub const BRACE_NEWLINE: &str = "brace_newline";
pub const TRAILING_COMMAS: &str = "trailing_commas";
pub const MEMBER_CHAIN_MIN_CALLS: &str = "member_chain_min_calls";
pub const PREPROCESSOR_SYMBOLS: &str = "preprocessor_symbols";

/// Defaults are `csharp/microsoft`: Allman braces, four spaces, width 120.
pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::line_width(120),
        shared::indent_size(4),
        shared::indent_style(),
        shared::final_newline(true),
        OptionDef::choice(
            BRACE_NEWLINE,
            &["all", "none"],
            "all",
            "Opening braces of types, members, blocks and lambdas: `all` puts each on its own \
             line (Allman); `none` keeps it on the line of the declaration or statement.",
        ),
        OptionDef::choice(
            TRAILING_COMMAS,
            &["preserve", "never", "multiline"],
            "preserve",
            "Trailing comma after the last element of initializers, collection expressions, enums \
             and switch expressions: `preserve` keeps what the source has; `never` removes it; \
             `multiline` adds one when the list is on several lines and removes it otherwise.",
        ),
        OptionDef::int(
            MEMBER_CHAIN_MIN_CALLS,
            3,
            (1, 1000),
            "A chain of member accesses and calls with at least this many calls is broken one \
             call per line when it does not fit on a line.",
        ),
        OptionDef::words(
            PREPROCESSOR_SYMBOLS,
            "Conditional compilation symbols (`DEBUG`, `NET8_0`) defined when parsing, which \
             decide the active branch of `#if`. Only the active branch is formatted.",
        ),
    ])
    .with_presets(presets())
}

/// Preset-internal value (not in the schema, so users cannot set it): `csharpier` selects the
/// layout variants that reproduce CSharpier's output; absent means the default layout.
const LAYOUT: &str = "layout";

/// `csharp/microsoft` is the schema's defaults. `csharp/csharpier-compat` reproduces CSharpier's
/// output (print width 100, its layout variants).
fn presets() -> Vec<Preset> {
    vec![
        Preset::new("csharp/microsoft@1", vec![]),
        Preset::new(
            "csharp/dotnet-runtime@1",
            vec![(LAYOUT, OptionValue::String("runtime".into()))],
        ),
        Preset::new(
            "csharp/google@1",
            vec![
                (shared::LINE_WIDTH, OptionValue::Int(100)),
                (shared::INDENT_SIZE, OptionValue::Int(2)),
                (BRACE_NEWLINE, OptionValue::String("none".into())),
                (TRAILING_COMMAS, OptionValue::String("multiline".into())),
                (LAYOUT, OptionValue::String("google".into())),
            ],
        ),
        Preset::new(
            "csharp/dotnet-runtime@1",
            vec![(LAYOUT, OptionValue::String("runtime".into()))],
        ),
        Preset::new(
            "csharp/google@1",
            vec![
                (shared::LINE_WIDTH, OptionValue::Int(100)),
                (shared::INDENT_SIZE, OptionValue::Int(2)),
                (BRACE_NEWLINE, OptionValue::String("none".into())),
                (TRAILING_COMMAS, OptionValue::String("multiline".into())),
                (LAYOUT, OptionValue::String("google".into())),
            ],
        ),
        Preset::new(
            "csharp/csharpier-compat@1",
            vec![
                (shared::LINE_WIDTH, OptionValue::Int(100)),
                (TRAILING_COMMAS, OptionValue::String("multiline".into())),
                (LAYOUT, OptionValue::String("csharpier".into())),
            ],
        ),
    ]
}

fn layout_is(options: &ResolvedOptions, name: &str) -> bool {
    matches!(options.get(LAYOUT), Some(OptionValue::String(s)) if s == name)
}

/// Does this file use CSharpier's layout variants (the `csharp/csharpier-compat` preset)?
pub fn csharpier(options: &ResolvedOptions) -> bool {
    layout_is(options, "csharpier")
}

/// `get { return x; }` stays on one line when it fits (`csharp/dotnet-runtime`, CSharpier).
pub fn inline_accessor_bodies(options: &ResolvedOptions) -> bool {
    csharpier(options) || layout_is(options, "runtime")
}

/// `} else {`, `} catch {`, `} finally {` (`csharp/google`).
pub fn cuddled_clauses(options: &ResolvedOptions) -> bool {
    layout_is(options, "google")
}

/// `new T {` with the brace on the line of the expression (`csharp/google`).
pub fn same_line_initializers(options: &ResolvedOptions) -> bool {
    layout_is(options, "google")
}

/// What stands between the braces of an empty body: `{ }`, or `{}` for `csharp/google`.
pub fn empty_braces(options: &ResolvedOptions) -> &'static str {
    if layout_is(options, "google") {
        ""
    } else {
        " "
    }
}

pub fn brace_on_new_line(options: &ResolvedOptions) -> bool {
    options.str(BRACE_NEWLINE) == "all"
}

/// Initializers and collection expressions: CSharpier lays them out by width alone, the default
/// keeps a list expanded when the source has a line break in it.
pub fn initializer_layout(options: &ResolvedOptions) -> ListLayout {
    if csharpier(options) {
        ListLayout::Fit
    } else {
        ListLayout::PreserveBreak
    }
}

/// How a list that may carry a trailing comma is printed, given whether its source had one.
pub fn trailing_comma(options: &ResolvedOptions, source_has_one: bool) -> TrailingComma {
    match options.str(TRAILING_COMMAS) {
        "never" => TrailingComma::Never,
        "multiline" => TrailingComma::WhenBroken,
        _ if source_has_one => TrailingComma::WhenBroken,
        _ => TrailingComma::Never,
    }
}
