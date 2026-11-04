//! Rules that need a symbol or a type from the worker (`symbol_findings`): each reports the
//! places one check of the worker found. `debug-assert-message` and `debug-fail-message` also
//! work without the worker, by name.

use super::{kind, range_of, text_of};
use core_rules::rule;
use core_rules::{Edit, FileContext, Fix, FixKind, RuleContext};
use core_semantics::Finding;
use core_syntax::SyntaxNode;

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

rule! {
    pub NoUnneededBasePrefix {
        id: "no-unneeded-base-prefix",
        category: Style,
        default: Off,
        fix: Safe,
        description: "`base.M()` or `base[i]` where the type does not override the member, so `this` binds to \
     the same one. The fix writes `this`.",
        example: "class B : A { void M() { base.Run(); } }   // no override of Run: write this.Run()",
        requires: [Types],
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

rule! {
    pub NoDefaultValueTypeConstructor {
        id: "no-default-value-type-constructor",
        category: Style,
        default: Off,
        fix: Safe,
        description: "`new S()` of a value type with an implicit parameterless constructor and no initializer: \
     write `default(S)` (`Guid.Empty`, `CancellationToken.None`, `IntPtr.Zero`, the enum member \
     with value 0 where they exist). No fix for a target-typed `new()`.",
        example: "var d = new DateTime();   // write: default(DateTime)",
        requires: [Types],
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

rule! {
    pub QualifyNamespaceUsings {
        id: "qualify-namespace-usings",
        category: Style,
        default: Off,
        fix: Safe,
        description: "A `using` directive inside a namespace whose name is not written fully qualified (it \
     resolves relative to the enclosing namespace). The fix writes the full name, unless it \
     has type arguments.",
        example: "namespace A.B { using C.D; }   // C.D is A.C.D here: write using A.C.D;",
        requires: [Types],
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

rule! {
    pub UseTupleSyntax {
        id: "use-tuple-syntax",
        category: Style,
        default: Off,
        fix: Safe,
        description: "`ValueTuple<int, int>` and `ValueTuple.Create(a, b)` where tuple syntax `(int, int)` and \
     `(a, b)` is available (not inside an expression tree). The fix is offered for types and \
     `ValueTuple.Create` calls.",
        example: "ValueTuple<int, int> p;   // write: (int, int) p;",
        requires: [Types],
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

rule! {
    pub TupleElementNamesInSignatures {
        id: "tuple-element-names-in-signatures",
        category: Style,
        default: Off,
        description: "A tuple type in the signature of a method, constructor, property, indexer, conversion \
     operator or delegate whose elements have no names. Overrides and implementations of \
     interface members are left alone (their names are not the member's to choose). No fix.",
        example: "(int, int) Pair();   // write: (int x, int y) Pair();",
        requires: [Types],
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

rule! {
    pub InheritdocOnInheritingMember {
        id: "inheritdoc-on-inheriting-member",
        category: Style,
        default: Off,
        description: "`<inheritdoc />` without `cref` on a type without a base list, or on a member that neither \
     overrides nor implements anything (a constructor whose signature matches a base \
     constructor is fine). No fix.",
        example: "/// <inheritdoc />\npublic void Run() { }   // implements nothing",
        requires: [Types],
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

rule! {
    pub DebugAssertMessage {
        id: "debug-assert-message",
        category: Style,
        default: Off,
        description: "`Debug.Assert` without a message, or with a constant blank one. With the type layer the \
     call is checked against `System.Diagnostics.Debug`; without it any call written \
     `Debug.Assert` is. No fix.",
        example: "Debug.Assert(ok);   // add a message",
        uses: [Types],
        kinds: &[kind::INVOCATION_EXPRESSION],
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

rule! {
    pub DebugFailMessage {
        id: "debug-fail-message",
        category: Style,
        default: Off,
        description: "`Debug.Fail` with a constant blank message. With the type layer the call is checked \
     against `System.Diagnostics.Debug`; without it any call written `Debug.Fail` is. No fix.",
        example: "Debug.Fail(\"\");   // say what went wrong",
        uses: [Types],
        kinds: &[kind::INVOCATION_EXPRESSION],
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
