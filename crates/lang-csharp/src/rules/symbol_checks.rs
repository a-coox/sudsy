//! Rules that need a symbol or a type from the worker (`symbol_findings`): each reports the
//! places one check of the worker found. `debug-assert-message` and `debug-fail-message` also
//! work without the worker, by name.

use super::{kind, range_of, text_of};
use core_config::Level;
use core_rules::{Category, Edit, FileContext, Fix, FixKind, Layer, Rule, RuleContext, RuleMeta};
use core_semantics::Finding;
use core_syntax::{SyntaxKind, SyntaxNode};

pub struct NoUnneededBasePrefix;
pub struct NoDefaultValueTypeConstructor;
pub struct QualifyNamespaceUsings;
pub struct UseTupleSyntax;
pub struct TupleElementNamesInSignatures;
pub struct DebugAssertMessage;
pub struct DebugFailMessage;
pub struct InheritdocOnInheritingMember;

const fn meta(
    id: &'static str,
    description: &'static str,
    example: &'static str,
    fix: Option<FixKind>,
    requires: &'static [Layer],
    uses: &'static [Layer],
) -> RuleMeta {
    RuleMeta {
        id,
        category: Category::Style,
        description,
        example,
        default_level: Level::Off,
        fix,
        requires,
        uses,
        options: &[],
    }
}

static BASE_PREFIX: RuleMeta = meta(
    "no-unneeded-base-prefix",
    "`base.M()` or `base[i]` where the type does not override the member, so `this` binds to \
     the same one. The fix writes `this`. Equivalent to StyleCop SA1100.",
    "class B : A { void M() { base.Run(); } }   // no override of Run: write this.Run()",
    Some(FixKind::Safe),
    &[Layer::Types],
    &[],
);
static DEFAULT_CTOR: RuleMeta = meta(
    "no-default-value-type-constructor",
    "`new S()` of a value type with an implicit parameterless constructor and no initializer: \
     write `default(S)` (`Guid.Empty`, `CancellationToken.None`, `IntPtr.Zero`, the enum member \
     with value 0 where they exist). No fix for a target-typed `new()`. Equivalent to StyleCop \
     SA1129.",
    "var d = new DateTime();   // write: default(DateTime)",
    Some(FixKind::Safe),
    &[Layer::Types],
    &[],
);
static QUALIFY_USINGS: RuleMeta = meta(
    "qualify-namespace-usings",
    "A `using` directive inside a namespace whose name is not written fully qualified (it \
     resolves relative to the enclosing namespace). The fix writes the full name, unless it \
     has type arguments. Equivalent to StyleCop SA1135.",
    "namespace A.B { using C.D; }   // C.D is A.C.D here: write using A.C.D;",
    Some(FixKind::Safe),
    &[Layer::Types],
    &[],
);
static TUPLE_SYNTAX: RuleMeta = meta(
    "use-tuple-syntax",
    "`ValueTuple<int, int>` and `ValueTuple.Create(a, b)` where tuple syntax `(int, int)` and \
     `(a, b)` is available (not inside an expression tree). The fix is offered for types and \
     `ValueTuple.Create` calls. Equivalent to StyleCop SA1141.",
    "ValueTuple<int, int> p;   // write: (int, int) p;",
    Some(FixKind::Safe),
    &[Layer::Types],
    &[],
);
static TUPLE_NAMES: RuleMeta = meta(
    "tuple-element-names-in-signatures",
    "A tuple type in the signature of a method, constructor, property, indexer, conversion \
     operator or delegate whose elements have no names. Overrides and implementations of \
     interface members are left alone (their names are not the member's to choose). No fix. \
     Equivalent to StyleCop SA1414.",
    "(int, int) Pair();   // write: (int x, int y) Pair();",
    None,
    &[Layer::Types],
    &[],
);
static DEBUG_ASSERT: RuleMeta = meta(
    "debug-assert-message",
    "`Debug.Assert` without a message, or with a constant blank one. With the type layer the \
     call is checked against `System.Diagnostics.Debug`; without it any call written \
     `Debug.Assert` is. No fix. Equivalent to StyleCop SA1405.",
    "Debug.Assert(ok);   // add a message",
    None,
    &[],
    &[Layer::Types],
);
static DEBUG_FAIL: RuleMeta = meta(
    "debug-fail-message",
    "`Debug.Fail` with a constant blank message. With the type layer the call is checked \
     against `System.Diagnostics.Debug`; without it any call written `Debug.Fail` is. No fix. \
     Equivalent to StyleCop SA1406.",
    "Debug.Fail(\"\");   // say what went wrong",
    None,
    &[],
    &[Layer::Types],
);
static INHERITDOC: RuleMeta = meta(
    "inheritdoc-on-inheriting-member",
    "`<inheritdoc />` without `cref` on a type without a base list, or on a member that neither \
     overrides nor implements anything (a constructor whose signature matches a base \
     constructor is fine). No fix. Equivalent to StyleCop SA1648.",
    "/// <inheritdoc />\npublic void Run() { }   // implements nothing",
    None,
    &[Layer::Types],
    &[],
);

/// Reports every finding of `check`; the fix replaces the finding's fix range (or its span).
fn report(
    file: &FileContext<'_>,
    ctx: &mut RuleContext<'_>,
    check: &str,
    message: impl Fn(&Finding) -> String,
    fix: Option<&str>,
) {
    let Some(view) = &file.semantics else { return };
    for f in view.data.answers.findings_of(check) {
        let range = range_of(f.start, f.len);
        let diagnostic = ctx.report(range, message(f));
        if let Some(label) = fix.filter(|_| !f.text.is_empty()) {
            let target = if f.fix_len == 0 {
                range
            } else {
                range_of(f.fix_start, f.fix_len)
            };
            diagnostic.fix(Fix::new(
                label,
                FixKind::Safe,
                vec![Edit::replace(target, f.text.clone())],
            ));
        }
    }
}

impl Rule for NoUnneededBasePrefix {
    fn meta(&self) -> &'static RuleMeta {
        &BASE_PREFIX
    }
    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "base_call",
            |_| "do not prefix the call with `base`: this type does not override the member".into(),
            Some("Use `this`"),
        );
    }
}

impl Rule for NoDefaultValueTypeConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &DEFAULT_CTOR
    }
    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "default_value_constructor",
            |f| match f.text.as_str() {
                "" => "do not use the default constructor of a value type: use `default`".into(),
                text => format!("do not use the default constructor of a value type: use `{text}`"),
            },
            Some("Replace the constructor call"),
        );
    }
}

impl Rule for QualifyNamespaceUsings {
    fn meta(&self) -> &'static RuleMeta {
        &QUALIFY_USINGS
    }
    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "unqualified_using",
            |f| format!("the using is not fully qualified (`{}`)", f.text),
            Some("Qualify the using"),
        );
    }
}

impl Rule for UseTupleSyntax {
    fn meta(&self) -> &'static RuleMeta {
        &TUPLE_SYNTAX
    }
    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "value_tuple",
            |_| "use tuple syntax instead of `ValueTuple`".into(),
            Some("Use tuple syntax"),
        );
    }
}

impl Rule for TupleElementNamesInSignatures {
    fn meta(&self) -> &'static RuleMeta {
        &TUPLE_NAMES
    }
    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "unnamed_tuple_element",
            |_| "tuple elements in a signature should have names".into(),
            None,
        );
    }
}

impl Rule for InheritdocOnInheritingMember {
    fn meta(&self) -> &'static RuleMeta {
        &INHERITDOC
    }
    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "inheritdoc",
            |_| "`inheritdoc` is only for members that override or implement something".into(),
            None,
        );
    }
}

/// `Debug.<method>(...)` written as such: the argument list's children, or `None`.
fn debug_call(node: &SyntaxNode, name: &str, ctx: &RuleContext<'_>) -> Option<Vec<SyntaxNode>> {
    let callee = node.children().next()?;
    if callee.kind() != kind::SIMPLE_MEMBER_ACCESS_EXPRESSION {
        return None;
    }
    let text: String = text_of(&callee, ctx)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let target = format!("Debug.{name}");
    (text == target || (text.ends_with(&format!(".{target}")) && text.starts_with("System."))).then(
        || {
            node.children()
                .find(|c| c.kind() == kind::ARGUMENT_LIST)
                .map(|list| {
                    list.children()
                        .filter(|c| c.kind() == kind::ARGUMENT)
                        .collect()
                })
                .unwrap_or_default()
        },
    )
}

/// Is the argument a string literal that is blank?
fn blank_literal(argument: &SyntaxNode, ctx: &RuleContext<'_>) -> bool {
    argument
        .children()
        .find(|c| c.kind() == kind::STRING_LITERAL_EXPRESSION)
        .is_some_and(|literal| {
            let text = text_of(&literal, ctx);
            let inner = text.trim_start_matches('@').trim_matches('"');
            inner.trim().is_empty()
        })
}

/// By name: the call is `Debug.<name>` with fewer than `index + 1` arguments or a blank literal.
fn debug_message(
    node: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
    name: &str,
    index: usize,
    message: &str,
) {
    if ctx.file.semantics.is_some() {
        return; // `check_file` has the exact answer
    }
    let Some(arguments) = debug_call(node, name, ctx) else {
        return;
    };
    let missing = arguments.len() <= index;
    if missing || blank_literal(&arguments[index], ctx) {
        ctx.report(
            core_rules::tree::significant_range(node, ctx.file.grammar),
            message,
        );
    }
}

impl Rule for DebugAssertMessage {
    fn meta(&self) -> &'static RuleMeta {
        &DEBUG_ASSERT
    }
    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::INVOCATION_EXPRESSION]
    }
    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        debug_message(
            node,
            ctx,
            "Assert",
            1,
            "Debug.Assert should provide message text",
        );
    }
    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "debug_assert_message",
            |_| "Debug.Assert should provide message text".into(),
            None,
        );
    }
}

impl Rule for DebugFailMessage {
    fn meta(&self) -> &'static RuleMeta {
        &DEBUG_FAIL
    }
    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::INVOCATION_EXPRESSION]
    }
    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        debug_message(
            node,
            ctx,
            "Fail",
            0,
            "Debug.Fail should provide message text",
        );
    }
    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "debug_fail_message",
            |_| "Debug.Fail should provide message text".into(),
            None,
        );
    }
}
