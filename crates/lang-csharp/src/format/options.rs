//! The C# format options: `brace_newline`, `trailing_commas` and `member_chain_min_calls` of
//! docs/formatting/csharp.md, the shared ones, and the layout choices the presets of
//! the plan's table differ in. Every option has a documented default; the defaults are
//! `csharp/microsoft`. Presets are config files (`presets/`): they only set these options, and
//! nothing in the formatter knows which preset is in use.

use crate::generated::kind as k;
use core_config::{OptionDef, OptionSchema, OptionValue, Preset, ResolvedOptions, shared};
use core_format::shapes::{ListLayout, TrailingComma};
use core_syntax::SyntaxKind;

pub const BRACE_NEWLINE: &str = "brace_newline";
pub const BRACE_SAME_LINE: &str = "brace_same_line";
pub const ELSE_LINE: &str = "else_line";
pub const CATCH_LINE: &str = "catch_line";
pub const FINALLY_LINE: &str = "finally_line";
pub const LABEL_INDENT: &str = "label_indent";
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
pub const CLOSING_PAREN_LINE: &str = "closing_paren_line";
pub const BLANK_LINES_AFTER_BLOCK: &str = "blank_lines_after_block";
pub const BLANK_LINE_BEFORE_COMMENT: &str = "blank_line_before_comment";
pub const BLANK_LINE_AFTER_COMMENT: &str = "blank_line_after_comment";
pub const SPACE_AFTER_DELEGATE: &str = "space_after_delegate";
pub const SPACE_BETWEEN_EMPTY_FOR_CLAUSES: &str = "space_between_empty_for_clauses";
pub const SORT_USINGS: &str = "sort_usings";

const CLAUSE_LINE_VALUES: &[&str] = &["inherit", "new_line", "same_line"];

/// Defaults are `csharp/microsoft`: Allman braces, four spaces, width 120.
pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::line_width(120),
        shared::indent_size(4),
        shared::indent_style(),
        shared::final_newline(true),
        shared::end_of_line(),
        OptionDef::choice(
            BRACE_NEWLINE,
            &["all", "none"],
            "all",
            "Opening braces of types, members, blocks and lambdas: `all` puts each on its own \
             line (Allman); `none` keeps it on the line of the declaration or statement.",
        ),
        OptionDef::words(
            BRACE_SAME_LINE,
            "Elements whose opening brace stays on the line of the declaration or statement \
             although `brace_newline` is `all`: `types`, `methods`, `properties`, `accessors`, \
             `anonymous_methods`, `anonymous_types`, `control_blocks`, `lambdas`, \
             `object_collection_array_initializers` (types include namespaces, methods include \
             constructors, operators and local functions, properties include indexers and \
             events, control blocks include `switch` statements and bare blocks).",
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
            &["preserve", "separate", "always"],
            "preserve",
            "Blank lines between the members of a type, namespace or enum: `preserve` keeps the \
             source's (at most one) and never adds one; `separate` also requires one between two \
             members when either has a body, an attribute or a comment above it; `always` \
             requires one between any two members of a type or namespace and between the \
             `using` directives and what follows them, except between two fields or two `using` \
             directives (enum members as `separate`).",
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
            ELSE_LINE,
            CLAUSE_LINE_VALUES,
            "inherit",
            "`else` after a block: `new_line` starts a line; `same_line` follows the closing \
             brace; `inherit` takes `clause_keyword_line`.",
        ),
        OptionDef::choice(
            CATCH_LINE,
            CLAUSE_LINE_VALUES,
            "inherit",
            "`catch` after a block: `new_line` starts a line; `same_line` follows the closing \
             brace; `inherit` takes `clause_keyword_line`.",
        ),
        OptionDef::choice(
            FINALLY_LINE,
            CLAUSE_LINE_VALUES,
            "inherit",
            "`finally` after a block: `new_line` starts a line; `same_line` follows the closing \
             brace; `inherit` takes `clause_keyword_line`.",
        ),
        OptionDef::choice(
            LABEL_INDENT,
            &["no_change", "one_less", "flush_left"],
            "no_change",
            "Indentation of a `goto` label: `no_change` keeps the indentation of the statements \
             around it; `one_less` indents it one level less; `flush_left` puts it in column 0.",
        ),
        OptionDef::choice(
            EMPTY_BRACES,
            &["space", "tight", "expanded"],
            "space",
            "Braces with nothing between them: `space` prints `{ }`, `tight` prints `{}`, \
             `expanded` puts the closing brace of an empty body on the line after the opening \
             one (initializers, and lambdas and anonymous methods, stay `{ }`).",
        ),
        OptionDef::choice(
            CLOSING_PAREN_LINE,
            &["own_line", "same_line"],
            "own_line",
            "The `)` of a parameter, argument or attribute argument list that does not fit on \
             one line: `own_line` puts it on a line of its own after the last item; `same_line` \
             right after the last item.",
        ),
        OptionDef::choice(
            BLANK_LINES_AFTER_BLOCK,
            &["preserve", "require"],
            "preserve",
            "Blank line after a statement, member or switch section that ends with a closing \
             brace: `preserve` keeps the source's (at most one); `require` adds one before the \
             next statement, member or section.",
        ),
        OptionDef::choice(
            BLANK_LINE_BEFORE_COMMENT,
            &["preserve", "require"],
            "preserve",
            "Blank line between a statement, member or switch section and a `//` comment on the \
             lines before the next one (not `////`): `preserve` keeps the source's; `require` \
             adds one.",
        ),
        OptionDef::choice(
            BLANK_LINE_AFTER_COMMENT,
            &["preserve", "remove"],
            "preserve",
            "Blank line between the last `//` or `///` comment in front of code and that code: \
             `preserve` keeps the source's (at most one); `remove` drops it. Comments at the \
             start of a file and `////` comments keep theirs.",
        ),
        OptionDef::boolean(
            SPACE_AFTER_DELEGATE,
            true,
            "A space between `delegate` and the parameter list of an anonymous method \
             (`delegate (int x) { }`); when false `delegate(int x) { }`.",
        ),
        OptionDef::boolean(
            SPACE_BETWEEN_EMPTY_FOR_CLAUSES,
            false,
            "A space between the two semicolons of a `for` with an empty condition \
             (`for (; ;)`); when false `for (;;)`.",
        ),
        OptionDef::boolean(
            SORT_USINGS,
            false,
            "Sort `using` directives like the `using-order` rule with              `compare: culture_ignore_case`, `static_position: after` and `alias_position:              after` (System first, then the others, `using static`, aliases), and join the              groups: blank lines between directives go, comments above a directive move with              it. Lists with preprocessor directives are left alone.",
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
        Preset::new(
            "csharp/stylecop",
            include_str!("../../presets/stylecop.yaml"),
        ),
    ]
}

fn is(options: &ResolvedOptions, name: &str, value: &str) -> bool {
    options.str(name) == value
}

/// Does the opening brace of this element (a name of `brace_same_line`) start a line?
pub fn brace_on_new_line(options: &ResolvedOptions, element: &str) -> bool {
    is(options, BRACE_NEWLINE, "all") && !brace_same_line(options, element)
}

fn brace_same_line(options: &ResolvedOptions, element: &str) -> bool {
    matches!(options.get(BRACE_SAME_LINE), Some(OptionValue::List(items))
        if items.iter().any(|i| matches!(i, OptionValue::String(s) if s == element)))
}

/// Indentation levels a label is moved out by, `usize::MAX` for column 0.
pub fn label_outdent(options: &ResolvedOptions) -> usize {
    match options.str(LABEL_INDENT) {
        "one_less" => 1,
        "flush_left" => usize::MAX,
        _ => 0,
    }
}

/// Members need a blank line between them when either has a body or something above it.
pub fn separate_members(options: &ResolvedOptions) -> bool {
    !is(options, BLANK_LINES_BETWEEN_MEMBERS, "preserve")
}

/// Every two members need a blank line between them (but two fields, two `using`s).
pub fn separate_all_members(options: &ResolvedOptions) -> bool {
    is(options, BLANK_LINES_BETWEEN_MEMBERS, "always")
}

/// Empty bodies put the closing brace on their own line.
pub fn expand_empty_bodies(options: &ResolvedOptions) -> bool {
    is(options, EMPTY_BRACES, "expanded")
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
    !is(options, WRAPPED_BRACE_LINE, "none") && !brace_same_line(options, INITIALIZERS)
}

/// The brace of a wrapped nested initializer or property pattern goes on its own line.
pub fn nested_brace_on_new_line(options: &ResolvedOptions) -> bool {
    is(options, WRAPPED_BRACE_LINE, "all") && !brace_same_line(options, INITIALIZERS)
}

/// The brace of a wrapped `new { ... }` (anonymous type) goes on its own line.
pub fn anonymous_type_brace_on_new_line(options: &ResolvedOptions) -> bool {
    !brace_same_line(options, "anonymous_types")
}

/// The element name of initializer braces in `brace_same_line`.
const INITIALIZERS: &str = "object_collection_array_initializers";

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

pub fn hug_closing_paren(options: &ResolvedOptions) -> bool {
    is(options, CLOSING_PAREN_LINE, "same_line")
}

pub fn blank_after_block(options: &ResolvedOptions) -> bool {
    is(options, BLANK_LINES_AFTER_BLOCK, "require")
}

pub fn blank_before_comment(options: &ResolvedOptions) -> bool {
    is(options, BLANK_LINE_BEFORE_COMMENT, "require")
}

pub fn attach_comments(options: &ResolvedOptions) -> bool {
    is(options, BLANK_LINE_AFTER_COMMENT, "remove")
}

pub fn space_after_delegate(options: &ResolvedOptions) -> bool {
    options.bool(SPACE_AFTER_DELEGATE)
}

pub fn sort_usings(options: &ResolvedOptions) -> bool {
    options.bool(SORT_USINGS)
}

pub fn space_between_empty_for_clauses(options: &ResolvedOptions) -> bool {
    options.bool(SPACE_BETWEEN_EMPTY_FOR_CLAUSES)
}

pub fn do_while_on_closing_line(options: &ResolvedOptions) -> bool {
    is(options, DO_WHILE_LINE, "same_line")
}

/// Does this `else`, `catch` or `finally` clause follow the closing brace of the block before?
pub fn clauses_on_closing_line(options: &ResolvedOptions, clause: SyntaxKind) -> bool {
    let own = match clause {
        k::ELSE_CLAUSE => ELSE_LINE,
        k::CATCH_CLAUSE => CATCH_LINE,
        _ => FINALLY_LINE,
    };
    match options.str(own) {
        "inherit" => is(options, CLAUSE_KEYWORD_LINE, "same_line"),
        line => line == "same_line",
    }
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
