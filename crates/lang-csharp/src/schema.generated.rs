// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const BRACE_NEWLINE: &str = "brace_newline";
#[allow(dead_code)]
pub const BRACE_SAME_LINE: &str = "brace_same_line";
#[allow(dead_code)]
pub const TRAILING_COMMAS: &str = "trailing_commas";
#[allow(dead_code)]
pub const MEMBER_CHAIN_MIN_CALLS: &str = "member_chain_min_calls";
#[allow(dead_code)]
pub const PREPROCESSOR_SYMBOLS: &str = "preprocessor_symbols";
#[allow(dead_code)]
pub const PREPROCESSOR_SYMBOL_SETS: &str = "preprocessor_symbol_sets";
#[allow(dead_code)]
pub const DERIVE_SYMBOL_SETS: &str = "derive_symbol_sets";
#[allow(dead_code)]
pub const BLANK_LINES_BETWEEN_MEMBERS: &str = "blank_lines_between_members";
#[allow(dead_code)]
pub const ACCESSOR_BLANK_LINES: &str = "accessor_blank_lines";
#[allow(dead_code)]
pub const COLLECTION_BLANK_LINES: &str = "collection_blank_lines";
#[allow(dead_code)]
pub const INLINE_ACCESSOR_BODIES: &str = "inline_accessor_bodies";
#[allow(dead_code)]
pub const PRESERVE_SINGLE_LINE_BLOCKS: &str = "preserve_single_line_blocks";
#[allow(dead_code)]
pub const PRESERVE_SINGLE_LINE_STATEMENTS: &str = "preserve_single_line_statements";
#[allow(dead_code)]
pub const INDENT_SWITCH_SECTION_BLOCK: &str = "indent_switch_section_block";
#[allow(dead_code)]
pub const ASSIGNMENT_LAYOUT: &str = "assignment_layout";
#[allow(dead_code)]
pub const SWITCH_ARM_LAYOUT: &str = "switch_arm_layout";
#[allow(dead_code)]
pub const INDENT_NESTED_BINARY: &str = "indent_nested_binary";
#[allow(dead_code)]
pub const MEMBER_CHAIN_KEEP_ACCESSES: &str = "member_chain_keep_accesses";
#[allow(dead_code)]
pub const MEMBER_CHAIN_BREAK_ON_BLOCK: &str = "member_chain_break_on_block";
#[allow(dead_code)]
pub const WRAPPED_BRACE_LINE: &str = "wrapped_brace_line";
#[allow(dead_code)]
pub const INITIALIZER_COLLAPSE: &str = "initializer_collapse";
#[allow(dead_code)]
pub const PRESERVE_QUERY_BREAKS: &str = "preserve_query_breaks";
#[allow(dead_code)]
pub const INITIALIZER_EXPAND: &str = "initializer_expand";
#[allow(dead_code)]
pub const LAMBDA_ARGUMENT_HUG: &str = "lambda_argument_hug";
#[allow(dead_code)]
pub const CONSTRUCTOR_INITIALIZER_LINE: &str = "constructor_initializer_line";
#[allow(dead_code)]
pub const PARAMETER_ATTRIBUTE_LAYOUT: &str = "parameter_attribute_layout";
#[allow(dead_code)]
pub const BASE_LIST_LAYOUT: &str = "base_list_layout";
#[allow(dead_code)]
pub const UNBREAKABLE_SINGLE_ITEM_LISTS: &str = "unbreakable_single_item_lists";
#[allow(dead_code)]
pub const DO_WHILE_LINE: &str = "do_while_line";
#[allow(dead_code)]
pub const CLAUSE_KEYWORD_LINE: &str = "clause_keyword_line";
#[allow(dead_code)]
pub const ELSE_LINE: &str = "else_line";
#[allow(dead_code)]
pub const CATCH_LINE: &str = "catch_line";
#[allow(dead_code)]
pub const FINALLY_LINE: &str = "finally_line";
#[allow(dead_code)]
pub const LABEL_INDENT: &str = "label_indent";
#[allow(dead_code)]
pub const EMPTY_BRACES: &str = "empty_braces";
#[allow(dead_code)]
pub const CLOSING_PAREN_LINE: &str = "closing_paren_line";
#[allow(dead_code)]
pub const BLANK_LINES_AFTER_BLOCK: &str = "blank_lines_after_block";
#[allow(dead_code)]
pub const BLANK_LINE_BEFORE_COMMENT: &str = "blank_line_before_comment";
#[allow(dead_code)]
pub const BLANK_LINE_AFTER_COMMENT: &str = "blank_line_after_comment";
#[allow(dead_code)]
pub const SPACE_AFTER_DELEGATE: &str = "space_after_delegate";
#[allow(dead_code)]
pub const SPACE_BETWEEN_EMPTY_FOR_CLAUSES: &str = "space_between_empty_for_clauses";
#[allow(dead_code)]
pub const SPACE_AFTER_EMPTY_LAST_FOR_CLAUSE: &str = "space_after_empty_last_for_clause";
#[allow(dead_code)]
pub const SORT_USINGS: &str = "sort_usings";
#[allow(dead_code)]
pub const SORT_USINGS_SYSTEM_FIRST: &str = "sort_usings_system_first";
#[allow(dead_code)]
pub const SEPARATE_USING_GROUPS: &str = "separate_using_groups";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::line_width(120),
        core_config::shared::indent_size(4),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::choice("brace_newline", &["all", "none"], "all", "Opening braces of types, members, blocks and lambdas: `all` puts each on its own line (Allman); `none` keeps it on the line of the declaration or statement."),
        core_config::OptionDef::words("brace_same_line", "Elements whose opening brace stays on the line of the declaration or statement although `brace_newline` is `all`: `types`, `methods`, `properties`, `accessors`, `anonymous_methods`, `anonymous_types`, `control_blocks`, `lambdas`, `object_collection_array_initializers` (types include namespaces, methods include constructors, operators and local functions, properties include indexers and events, control blocks include `switch` statements and bare blocks)."),
        core_config::OptionDef::choice("trailing_commas", &["preserve", "never", "multiline"], "preserve", "Trailing comma after the last element of initializers, collection expressions, enums and switch expressions: `preserve` keeps what the source has; `never` removes it; `multiline` adds one when the list is on several lines and removes it otherwise."),
        core_config::OptionDef::int("member_chain_min_calls", 3, (1, 1000), "A chain of member accesses and calls with at least this many calls is broken one call per line when it does not fit on a line."),
        core_config::OptionDef::words("preprocessor_symbols", "Conditional compilation symbols (`DEBUG`, `NET8_0`) defined when parsing, which decide the active branch of `#if`. Branches that are inactive under these symbols stay as they are unless `preprocessor_symbol_sets` or `derive_symbol_sets` format them."),
        core_config::OptionDef::word_sets("preprocessor_symbol_sets", "Further sets of conditional compilation symbols. The file is also parsed with each set (a set replaces `preprocessor_symbols`), and the `#if` branches that are inactive under `preprocessor_symbols` but active under a set are formatted from that parse. Empty (the default) leaves inactive branches as they are."),
        core_config::OptionDef::boolean("derive_symbol_sets", false, "Also parse with sets of symbols taken from the `#if` and `#elif` conditions of the file (the empty set, all symbols, then each symbol, then pairs and so on, at most 16 sets, after those of `preprocessor_symbol_sets`) until every branch has been active once, and format the branches that are inactive under `preprocessor_symbols` from those parses. A branch no set activates stays as it is."),
        core_config::OptionDef::choice("blank_lines_between_members", &["preserve", "separate", "always"], "preserve", "Blank lines between the members of a type, namespace or enum: `preserve` keeps the source's (at most one) and never adds one; `separate` also requires one between two members when either has a body, an attribute or a comment above it; `always` requires one between any two members of a type or namespace and between the `using` directives and what follows them, except between two fields or two `using` directives (enum members as `separate`)."),
        core_config::OptionDef::choice("accessor_blank_lines", &["preserve", "remove"], "preserve", "Blank lines between the accessors of a property that is laid out on several lines: `preserve` keeps the source's (at most one); `remove` drops them."),
        core_config::OptionDef::choice("collection_blank_lines", &["preserve", "remove"], "preserve", "Blank lines between the elements of a collection initializer, array initializer or collection expression: `preserve` keeps the source's (at most one); `remove` drops them. The members of an object initializer keep theirs."),
        core_config::OptionDef::boolean("inline_accessor_bodies", false, "Keep the body of an accessor on its line (`get { return x; }`) when it is one simple statement without comments and fits; otherwise the braces go on their own lines."),
        core_config::OptionDef::boolean("preserve_single_line_blocks", false, "A block written on one line (`void M() { return; }`, `get { return x; }`,              `if (a)` followed by `{ b(); }`) stays on one line, as `{ a; b; }`, when it has no comment and no label in it. Type, namespace, enum and `switch` bodies written on one line stay as well."),
        core_config::OptionDef::boolean("preserve_single_line_statements", false, "Statements written on one line stay there: `a(); b();` in a sequence, `case 1:              break;`, and a whole `if`, `while`, `for`, `foreach`, `do`, `try`, `using`, `lock`,              `fixed`, `checked` or `unsafe` statement (with its blocks) written on one line,              unless it has a comment, a `switch` or a label in it."),
        core_config::OptionDef::boolean("indent_switch_section_block", true, "Indent a block that is the only statement of a `case` section below its label; when false the block starts at the label's indentation."),
        core_config::OptionDef::choice("assignment_layout", &["compact", "fluid"], "compact", "Where the value after `=` or the body after `=>` goes when the line is too long: `compact` moves a binary expression, a conditional or a query to the next line and leaves any other value on the line; `fluid` also moves strings, collection expressions and property initializers, and any other value whose first line does not fit after the operator; a string that spans lines stays on the operator's              line, and the body of an expression lambda moves below `=>` when the lambda does              not fit."),
        core_config::OptionDef::choice("switch_arm_layout", &["inline", "assignment"], "inline", "Layout of a switch expression arm: `inline` keeps `pattern => value` as one run of tokens; `assignment` lays out the value like the right-hand side of `=` (see `assignment_layout`)."),
        core_config::OptionDef::boolean("indent_nested_binary", false, "Indent the operands of a binary expression that is itself an operand of a binary expression with an operator of another precedence (`a && b || c`)."),
        core_config::OptionDef::boolean("member_chain_keep_accesses", false, "In a broken member chain a property access stays on the line of the call after it (`.A.B()` is one group) instead of starting a line of its own."),
        core_config::OptionDef::boolean("member_chain_break_on_block", false, "Break a member chain one call per line when a call other than the last has an argument with a block body (a lambda), even if the chain would fit."),
        core_config::OptionDef::choice("wrapped_brace_line", &["creation", "all", "none"], "creation", "Opening brace of an initializer or property pattern that is laid out on several lines: `creation` puts it on its own line for `new T { ... }` only; `all` also for an initializer nested in an assignment (`Prop = { ... }`) and for property patterns; `none` keeps it on the line of the expression."),
        core_config::OptionDef::choice("initializer_collapse", &["preserve", "always"], "preserve", "Initializers and collection expressions: `preserve` keeps one expanded when the source has a line break in it; `always` puts one on a single line whenever it fits."),
        core_config::OptionDef::boolean("preserve_query_breaks", false, "A query expression with a line break in the source keeps one clause per line even when it fits on a line."),
        core_config::OptionDef::choice("initializer_expand", &["fit", "crowded"], "fit", "Initializers (and anonymous types) that fit on a line: `fit` keeps them on it; `crowded` puts one item per line when there are three or more member assignments, or two or more items one of which creates an object with an initializer."),
        core_config::OptionDef::choice("lambda_argument_hug", &["last", "sole"], "last", "A lambda argument whose body can hang from the call's line: `last` hugs a block lambda that is the last argument; `sole` hugs a lambda (block or expression) only when it is the only argument."),
        core_config::OptionDef::choice("constructor_initializer_line", &["same_line", "new_line"], "same_line", "The `: base(...)` or `: this(...)` initializer of a constructor: `same_line` keeps it after the parameter list; `new_line` puts it on its own indented line."),
        core_config::OptionDef::choice("parameter_attribute_layout", &["inline", "hanging"], "inline", "A parameter with attribute lists: `inline` keeps them in front of the parameter;              `hanging` keeps them there when the parameter fits and otherwise puts the rest of              the parameter on an indented line below them."),
        core_config::OptionDef::choice("base_list_layout", &["inline", "hanging"], "inline", "The base list of a type (`: A, B`): `inline` keeps it on the line of the type name; `hanging` keeps it there when it fits and otherwise starts a line, with one base type per line."),
        core_config::OptionDef::boolean("unbreakable_single_item_lists", false, "Never break a list that holds one simple item: an attribute list with one attribute, a type argument list with one name, an index that is one name or literal."),
        core_config::OptionDef::choice("do_while_line", &["new_line", "same_line"], "new_line", "The `while` of a `do` statement: `new_line` starts a line; `same_line` follows the closing brace (`} while (x);`)."),
        core_config::OptionDef::choice("clause_keyword_line", &["new_line", "same_line"], "new_line", "`else`, `catch` and `finally` after a block: `new_line` starts a line; `same_line` follows the closing brace (`} else {`) unless a comment sits between them."),
        core_config::OptionDef::choice("else_line", &["inherit", "new_line", "same_line"], "inherit", "`else` after a block: `new_line` starts a line; `same_line` follows the closing brace; `inherit` takes `clause_keyword_line`."),
        core_config::OptionDef::choice("catch_line", &["inherit", "new_line", "same_line"], "inherit", "`catch` after a block: `new_line` starts a line; `same_line` follows the closing brace; `inherit` takes `clause_keyword_line`."),
        core_config::OptionDef::choice("finally_line", &["inherit", "new_line", "same_line"], "inherit", "`finally` after a block: `new_line` starts a line; `same_line` follows the closing brace; `inherit` takes `clause_keyword_line`."),
        core_config::OptionDef::choice("label_indent", &["no_change", "one_less", "flush_left"], "no_change", "Indentation of a `goto` label: `no_change` keeps the indentation of the statements around it; `one_less` indents it one level less; `flush_left` puts it in column 0."),
        core_config::OptionDef::choice("empty_braces", &["space", "tight", "expanded"], "space", "Braces with nothing between them: `space` prints `{ }`, `tight` prints `{}`, `expanded` puts the closing brace of an empty body on the line after the opening one (initializers, and lambdas and anonymous methods, stay `{ }`)."),
        core_config::OptionDef::choice("closing_paren_line", &["own_line", "same_line"], "own_line", "The `)` of a parameter, argument or attribute argument list that does not fit on one line: `own_line` puts it on a line of its own after the last item; `same_line` right after the last item."),
        core_config::OptionDef::choice("blank_lines_after_block", &["preserve", "require"], "preserve", "Blank line after a statement, member or switch section that ends with a closing brace: `preserve` keeps the source's (at most one); `require` adds one before the next statement, member or section."),
        core_config::OptionDef::choice("blank_line_before_comment", &["preserve", "require"], "preserve", "Blank line between a statement, member or switch section and a `//` comment on the lines before the next one (not `////`): `preserve` keeps the source's; `require` adds one."),
        core_config::OptionDef::choice("blank_line_after_comment", &["preserve", "remove"], "preserve", "Blank line between the last `//` or `///` comment in front of code and that code: `preserve` keeps the source's (at most one); `remove` drops it. Comments at the start of a file and `////` comments keep theirs."),
        core_config::OptionDef::boolean("space_after_delegate", true, "A space between `delegate` and the parameter list of an anonymous method (`delegate (int x) { }`); when false `delegate(int x) { }`."),
        core_config::OptionDef::boolean("space_between_empty_for_clauses", false, "A space between the two semicolons of a `for` with an empty condition (`for (; ;)`); when false `for (;;)`."),
        core_config::OptionDef::boolean("space_after_empty_last_for_clause", false, "A space before the `)` of a `for` whose last clause is empty (`for (int i = 0; i < n; )`); when false `for (int i = 0; i < n;)`."),
        core_config::OptionDef::boolean("sort_usings", false, "Sort `using` directives like the `using-order` rule with `compare: culture_ignore_case`, `static_position: after` and `alias_position: after` (System first, then the others, `using static`, aliases), and join the groups: blank lines between directives go, comments above a directive move with it. `global using` directives come first, `using global::X` behind the System ones. `#if` regions stay as they are behind the sorted directives; a list with another preprocessor directive is left alone."),
        core_config::OptionDef::boolean("sort_usings_system_first", true, "With `sort_usings`: `System` namespaces come before the others (`using static System.*` before other `using static`)."),
        core_config::OptionDef::boolean("separate_using_groups", false, "With `sort_usings`: one blank line between the System usings and the others (only with `sort_usings_system_first`)."),
    ])
    .with_presets(vec![
        core_config::Preset::new("csharp/microsoft", include_str!("../presets/microsoft.yaml")),
        core_config::Preset::new("csharp/dotnet-runtime", include_str!("../presets/dotnet-runtime.yaml")),
        core_config::Preset::new("csharp/google", include_str!("../presets/google.yaml")),
        core_config::Preset::new("csharp/csharpier-compat", include_str!("../presets/csharpier-compat.yaml")),
        core_config::Preset::new("csharp/stylecop", include_str!("../presets/stylecop.yaml")),
    ])
}
