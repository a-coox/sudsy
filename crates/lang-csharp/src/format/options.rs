//! The C# format options: `brace_newline`, `trailing_commas` and `member_chain_min_calls` of
//! docs/formatting/csharp.md, the shared ones, and the layout choices the presets of
//! the plan's table differ in. Every option has a documented default; the defaults are
//! `csharp/microsoft`. Presets are config files (`presets/`): they only set these options, and
//! nothing in the formatter knows which preset is in use.

use crate::generated::kind as k;
use core_config::{OptionValue, ResolvedOptions, shared};
use core_format::shapes::{ListLayout, TrailingComma};
use core_syntax::SyntaxKind;

include!("../schema.generated.rs");

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

/// Blank lines allowed between the elements of a collection (not object) initializer.
pub fn collection_blank_lines(options: &ResolvedOptions) -> usize {
    usize::from(!is(options, COLLECTION_BLANK_LINES, "remove"))
}

pub fn remove_accessor_blank_lines(options: &ResolvedOptions) -> bool {
    is(options, ACCESSOR_BLANK_LINES, "remove")
}

pub fn inline_accessor_bodies(options: &ResolvedOptions) -> bool {
    options.bool(INLINE_ACCESSOR_BODIES)
}

pub fn preserve_single_line_blocks(options: &ResolvedOptions) -> bool {
    options.bool(PRESERVE_SINGLE_LINE_BLOCKS)
}

pub fn preserve_single_line_statements(options: &ResolvedOptions) -> bool {
    options.bool(PRESERVE_SINGLE_LINE_STATEMENTS)
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

pub fn preserve_query_breaks(options: &ResolvedOptions) -> bool {
    options.bool(PRESERVE_QUERY_BREAKS)
}

pub fn expand_crowded_initializers(options: &ResolvedOptions) -> bool {
    is(options, INITIALIZER_EXPAND, "crowded")
}

pub fn hug_only_sole_lambda(options: &ResolvedOptions) -> bool {
    is(options, LAMBDA_ARGUMENT_HUG, "sole")
}

pub fn hanging_parameter_attributes(options: &ResolvedOptions) -> bool {
    is(options, PARAMETER_ATTRIBUTE_LAYOUT, "hanging")
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

pub fn using_order(options: &ResolvedOptions) -> (crate::using_sort::Order, bool) {
    (
        crate::using_sort::Order::formatter(options.bool(SORT_USINGS_SYSTEM_FIRST)),
        options.bool(SEPARATE_USING_GROUPS),
    )
}

pub fn space_after_empty_last_for_clause(options: &ResolvedOptions) -> bool {
    options.bool(SPACE_AFTER_EMPTY_LAST_FOR_CLAUSE)
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
