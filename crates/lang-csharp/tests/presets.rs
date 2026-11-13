//! Presets: `csharp/csharpier-compat` reproduces
//! CSharpier's. Every case goes through the shared checks of `common::fmt_preset`.

use crate::common::fmt_preset;

const COMPAT: &str = "csharp/csharpier-compat";

macro_rules! compat {
    ($name:ident, $input:expr) => {
        #[test]
        fn $name() {
            let input: &str = $input;
            let out = fmt_preset(input, COMPAT);
            insta::assert_snapshot!(format!("{input}\n-----\n{out}"));
        }
    };
}

#[test]
fn presets_are_resolved_from_extends() {
    let schema = lang_csharp::option_schema();
    let schemas = [("csharp", &schema)];
    let resolve = |text: &str| -> Result<core_config::Resolved, core_config::ConfigError> {
        let chain = core_config::ConfigChain::parse(text)?;
        chain.validate(&schemas)?;
        chain.format_options(&core_config::FileInput::language("csharp"), &schema)
    };
    let options = resolve(
        "extends:
  - preset:csharp/csharpier-compat
",
    )
    .unwrap()
    .options;
    assert_eq!(options.int("line_width"), 100);
    assert_eq!(options.str("trailing_commas"), "multiline");
    // The user's sections still win over the preset.
    let options = resolve(
        "extends: [\"preset:csharp/csharpier-compat\"]
csharp:
  format:
    line_width: 80
",
    )
    .unwrap()
    .options;
    assert_eq!(options.int("line_width"), 80);
    let error = resolve(
        "extends: [\"preset:csharp/nope\"]
",
    )
    .unwrap_err();
    assert!(error.message.contains("csharp/csharpier-compat"), "{error}");
    // Every preset of the table in docs/formatting/presets.md resolves.
    for name in ["dotnet-runtime", "google", "csharpier-compat", "stylecop"] {
        resolve(&format!("extends: [\"preset:csharp/{name}\"]\n")).unwrap();
    }
}

compat!(
    compat_members_and_accessors,
    "class C : IFoo {\n private int a;\n private int b;\n [Obsolete] private int c;\n public int X { get { return a; } set { a = value; } }\n public int Y { get; set; }\n public int Z => 1;\n public C(int a) : base(a) { }\n public void M() { }\n void N();\n}"
);
compat!(
    compat_long_signatures,
    "public abstract partial class Array : ICloneable, IList, IStructuralComparable, IStructuralEquatable, IFoo { }\nclass D { public IDictionary<string, AuthenticationSchemeBuilder> SchemeMap { get; } = new Dictionary<string, AuthenticationSchemeBuilder>(StringComparer.Ordinal);\n protected Task InvokeAsync(Func<Task> workItem) => _renderHandle.Dispatcher.InvokeAsync(workItem, anotherArgument);\n}"
);
compat!(
    compat_statements,
    "class C { void M() {\n switch (x) { case 1: { Foo(); break; } default: break; }\n var d = new Dictionary<string, int> { [\"a\"] = 1 };\n var e = new Foo {\n A = 1,\n B = 2 };\n Assert.Throws<ArgumentNullException>(() => RequestDelegateFactory.Create(handler: null!, options: someVeryLongOptionsValue));\n do { x++; } while (x < 3);\n var s = \"a very long string literal that does not fit on the line with the declaration of the variable\";\n } }"
);

compat!(
    compat_trailing_comment_after_lambda_parameters,
    "class C { string M() {
 return string.Create(
 12, (IntPtr)pKey, (span, key) => // 12 == 8 + 1 (for period) + 3
 Populate83FileNameFromRandomBytes((byte*)key, KeyLength, span));
 } }"
);

compat!(
    compat_trailing_comment_after_last_switch_arm,
    "class C { void M() {
 var parameterTypeName = (a, b) switch {
 (true, true) => Extensions.ComplexArrayReceiverTypeName, // c
 (false, false) => parameterTypeInfo.Name // simple non-array type, e.g. \"int\"
 };
 } }"
);

#[test]
fn nested_hugged_lambdas_do_not_grow_exponentially() {
    // Every level of a `conditional_group` used to copy its candidates, doubling the document
    // size per level of nesting.
    let mut input = String::from("class C { void M() {\n");
    for _ in 0..24 {
        input.push_str("Run(() => {\n");
    }
    input.push_str("x();\n");
    for _ in 0..24 {
        input.push_str("});\n");
    }
    input.push_str("} }");
    crate::common::fmt("class C { }", &[]); // builds and starts the worker outside the timed section
    let started = std::time::Instant::now();
    let out = crate::common::fmt(&input, &[]);
    assert!(out.contains("Run(() =>"));
    assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
}

#[test]
fn stylecop_preset_lays_out_members_statements_and_lists() {
    let input = "using System;\nnamespace N\n{\n    // comment\n\n    public class C { private int a; private int b; public C() { }\n        public void M(int first, int second, int third, int fourth, int fifth, int sixth, int seventh, int eighth, int ninth)\n        { if (a == 1) { b++; }\n            b--;\n            for (;;) { break; }\n            Action<int> d = delegate (int v) { };\n        } }\n}\n";
    insta::assert_snapshot!(format!(
        "{input}\n-----\n{}",
        fmt_preset(input, "csharp/stylecop")
    ));
}
