//! The C# format options: `brace_newline`, `trailing_commas` and `member_chain_min_calls` of
//! docs/formatting/csharp.md, the shared ones, and the layout choices the presets of
//! the plan's table differ in. Every option has a documented default; the defaults are
//! `csharp/microsoft`. Presets are config files (`presets/`): they only set these options, and
//! nothing in the formatter knows which preset is in use.

use core_config::{OptionDef, OptionSchema, Preset, ResolvedOptions, shared};
use core_format::shapes::{ListLayout, TrailingComma};

pub const BRACE_NEWLINE: &str = "brace_newline";
pub const TRAILING_COMMAS: &str = "trailing_commas";
pub const MEMBER_CHAIN_MIN_CALLS: &str = "member_chain_min_calls";
pub const PREPROCESSOR_SYMBOLS: &str = "preprocessor_symbols";
pub const BLANK_LINES_BETWEEN_MEMBERS: &str = "blank_lines_between_members";
pub const ACCESSOR_BLANK_LINES: &str = "accessor_blank_lines";
pub const INLINE_ACCESSOR_BODIES: &str = "inline_accessor_bodies";
pub const INDENT_SWITCH_SECTION_BLOCK: &str = "indent_switch_section_block";
pub const ASSIGNMENT_LAYOUT: &str = "assignment_layout";
pub const SWITCH_ARM_LAYOUT: &str = "switch_arm_layout";
pub const INDENT_NESTED_BINARY: &str = "indent_nested_binary";
pub const MEMBER_CHAIN_KEEP_ACCESSES: &str = "member_chain_keep_accesses";
pub const MEMBER_CHAIN_BREAK_ON_BLOCK: &str = "member_chain_break_on_block";
pub const WRAPPED_BRACE_LINE: &str = "wrapped_brace_line";
pub const INITIALIZER_COLLAPSE: &str = "initializer_collapse";
pub const LAMBDA_ARGUMENT_HUG: &str = "lambda_argument_hug";
pub const CONSTRUCTOR_INITIALIZER_LINE: &str = "constructor_initializer_line";
pub const BASE_LIST_LAYOUT: &str = "base_list_layout";
pub const UNBREAKABLE_SINGLE_ITEM_LISTS: &str = "unbreakable_single_item_lists";
pub const DO_WHILE_LINE: &str = "do_while_line";
pub const CLAUSE_KEYWORD_LINE: &str = "clause_keyword_line";
pub const EMPTY_BRACES: &str = "empty_braces";

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
        OptionDef::choice(
            BLANK_LINES_BETWEEN_MEMBERS,
            &["preserve", "separate"],
            "preserve",
            "Blank lines between the members of a type, namespace or enum: `preserve` keeps the \
             source's (at most one) and never adds one; `separate` also requires one between two \
             members when either has a body, an attribute or a comment above it.",
        ),
        OptionDef::choice(
            ACCESSOR_BLANK_LINES,
            &["preserve", "remove"],
            "preserve",
            "Blank lines between the accessors of a property that is laid out on several lines: \
             `preserve` keeps the source's (at most one); `remove` drops them.",
        ),
        OptionDef::boolean(
            INLINE_ACCESSOR_BODIES,
            false,
            "Keep the body of an accessor on its line (`get { return x; }`) when it is one simple \
             statement without comments and fits; otherwise the braces go on their own lines.",
        ),
        OptionDef::boolean(
            INDENT_SWITCH_SECTION_BLOCK,
            true,
            "Indent a block that is the only statement of a `case` section below its label; when \
             false the block starts at the label's indentation.",
        ),
        OptionDef::choice(
            ASSIGNMENT_LAYOUT,
            &["compact", "fluid"],
            "compact",
            "Where the value after `=` or the body after `=>` goes when the line is too long: \
             `compact` moves a binary expression, a conditional or a query to the next line and \
             leaves any other value on the line; `fluid` also moves strings, collection \
             expressions and property initializers, and any other value whose first line does \
             not fit after the operator.",
        ),
        OptionDef::choice(
            SWITCH_ARM_LAYOUT,
            &["inline", "assignment"],
            "inline",
            "Layout of a switch expression arm: `inline` keeps `pattern => value` as one run of \
             tokens; `assignment` lays out the value like the right-hand side of `=` \
             (see `assignment_layout`).",
        ),
        OptionDef::boolean(
            INDENT_NESTED_BINARY,
            false,
            "Indent the operands of a binary expression that is itself an operand of a binary \
             expression with an operator of another precedence (`a && b || c`).",
        ),
        OptionDef::boolean(
            MEMBER_CHAIN_KEEP_ACCESSES,
            false,
            "In a broken member chain a property access stays on the line of the call after it \
             (`.A.B()` is one group) instead of starting a line of its own.",
        ),
        OptionDef::boolean(
            MEMBER_CHAIN_BREAK_ON_BLOCK,
            false,
            "Break a member chain one call per line when a call other than the last has an \
             argument with a block body (a lambda), even if the chain would fit.",
        ),
        OptionDef::choice(
            WRAPPED_BRACE_LINE,
            &["creation", "all", "none"],
            "creation",
            "Opening brace of an initializer or property pattern that is laid out on several \
             lines: `creation` puts it on its own line for `new T { ... }` only; `all` also for \
             an initializer nested in an assignment (`Prop = { ... }`) and for property patterns; \
             `none` keeps it on the line of the expression.",
        ),
        OptionDef::choice(
            INITIALIZER_COLLAPSE,
            &["preserve", "always"],
            "preserve",
            "Initializers and collection expressions: `preserve` keeps one expanded when the \
             source has a line break in it; `always` puts one on a single line whenever it fits.",
        ),
        OptionDef::choice(
            LAMBDA_ARGUMENT_HUG,
            &["last", "sole"],
            "last",
            "A lambda argument whose body can hang from the call's line: `last` hugs a block \
             lambda that is the last argument; `sole` hugs a lambda (block or expression) only \
             when it is the only argument.",
        ),
        OptionDef::choice(
            CONSTRUCTOR_INITIALIZER_LINE,
            &["same_line", "new_line"],
            "same_line",
            "The `: base(...)` or `: this(...)` initializer of a constructor: `same_line` keeps \
             it after the parameter list; `new_line` puts it on its own indented line.",
        ),
        OptionDef::choice(
            BASE_LIST_LAYOUT,
            &["inline", "hanging"],
            "inline",
            "The base list of a type (`: A, B`): `inline` keeps it on the line of the type name; \
             `hanging` keeps it there when it fits and otherwise starts a line, with one base \
             type per line.",
        ),
        OptionDef::boolean(
            UNBREAKABLE_SINGLE_ITEM_LISTS,
            false,
            "Never break a list that holds one simple item: an attribute list with one \
             attribute, a type argument list with one name, an index that is one name or \
             literal.",
        ),
        OptionDef::choice(
            DO_WHILE_LINE,
            &["new_line", "same_line"],
            "new_line",
            "The `while` of a `do` statement: `new_line` starts a line; `same_line` follows the \
             closing brace (`} while (x);`).",
        ),
        OptionDef::choice(
            CLAUSE_KEYWORD_LINE,
            &["new_line", "same_line"],
            "new_line",
            "`else`, `catch` and `finally` after a block: `new_line` starts a line; `same_line` \
             follows the closing brace (`} else {`) unless a comment sits between them.",
        ),
        OptionDef::choice(
            EMPTY_BRACES,
            &["space", "tight"],
            "space",
            "Braces with nothing between them: `space` prints `{ }`, `tight` prints `{}`.",
        ),
    ])
    .with_presets(presets())
}

/// The presets are plain config files in `presets/` next to the crate's manifest.
fn presets() -> Vec<Preset> {
    vec![
        Preset::new(
            "csharp/microsoft",
            include_str!("../../presets/microsoft.yaml"),
        ),
        Preset::new(
            "csharp/dotnet-runtime",
            include_str!("../../presets/dotnet-runtime.yaml"),
        ),
        Preset::new("csharp/google", include_str!("../../presets/google.yaml")),
        Preset::new(
            "csharp/csharpier-compat",
            include_str!("../../presets/csharpier-compat.yaml"),
        ),
    ]
}

fn is(options: &ResolvedOptions, name: &str, value: &str) -> bool {
    options.str(name) == value
}

pub fn brace_on_new_line(options: &ResolvedOptions) -> bool {
    is(options, BRACE_NEWLINE, "all")
}

/// Members need a blank line between them when either has a body or something above it.
pub fn separate_members(options: &ResolvedOptions) -> bool {
    is(options, BLANK_LINES_BETWEEN_MEMBERS, "separate")
}

pub fn remove_accessor_blank_lines(options: &ResolvedOptions) -> bool {
    is(options, ACCESSOR_BLANK_LINES, "remove")
}

pub fn inline_accessor_bodies(options: &ResolvedOptions) -> bool {
    options.bool(INLINE_ACCESSOR_BODIES)
}

pub fn indent_switch_section_block(options: &ResolvedOptions) -> bool {
    options.bool(INDENT_SWITCH_SECTION_BLOCK)
}

pub fn fluid_assignments(options: &ResolvedOptions) -> bool {
    is(options, ASSIGNMENT_LAYOUT, "fluid")
}

pub fn switch_arm_as_assignment(options: &ResolvedOptions) -> bool {
    is(options, SWITCH_ARM_LAYOUT, "assignment")
}

pub fn indent_nested_binary(options: &ResolvedOptions) -> bool {
    options.bool(INDENT_NESTED_BINARY)
}

pub fn keep_accesses_with_call(options: &ResolvedOptions) -> bool {
    options.bool(MEMBER_CHAIN_KEEP_ACCESSES)
}

pub fn chain_break_on_block(options: &ResolvedOptions) -> bool {
    options.bool(MEMBER_CHAIN_BREAK_ON_BLOCK)
}

/// The brace of a wrapped `new T { ... }` goes on its own line.
pub fn creation_brace_on_new_line(options: &ResolvedOptions) -> bool {
    !is(options, WRAPPED_BRACE_LINE, "none")
}

/// The brace of a wrapped nested initializer or property pattern goes on its own line.
pub fn nested_brace_on_new_line(options: &ResolvedOptions) -> bool {
    is(options, WRAPPED_BRACE_LINE, "all")
}

/// Initializers and collection expressions.
pub fn initializer_layout(options: &ResolvedOptions) -> ListLayout {
    if is(options, INITIALIZER_COLLAPSE, "always") {
        ListLayout::Fit
    } else {
        ListLayout::PreserveBreak
    }
}

pub fn hug_only_sole_lambda(options: &ResolvedOptions) -> bool {
    is(options, LAMBDA_ARGUMENT_HUG, "sole")
}

pub fn constructor_initializer_on_new_line(options: &ResolvedOptions) -> bool {
    is(options, CONSTRUCTOR_INITIALIZER_LINE, "new_line")
}

pub fn hanging_base_list(options: &ResolvedOptions) -> bool {
    is(options, BASE_LIST_LAYOUT, "hanging")
}

pub fn unbreakable_single_item_lists(options: &ResolvedOptions) -> bool {
    options.bool(UNBREAKABLE_SINGLE_ITEM_LISTS)
}

pub fn do_while_on_closing_line(options: &ResolvedOptions) -> bool {
    is(options, DO_WHILE_LINE, "same_line")
}

pub fn clauses_on_closing_line(options: &ResolvedOptions) -> bool {
    is(options, CLAUSE_KEYWORD_LINE, "same_line")
}

/// What stands between the braces of an empty body.
pub fn empty_braces(options: &ResolvedOptions) -> &'static str {
    if is(options, EMPTY_BRACES, "tight") {
        ""
    } else {
        " "
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
