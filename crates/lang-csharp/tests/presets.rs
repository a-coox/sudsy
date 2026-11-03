//! Presets: `csharp/microsoft` is the default layout, `csharp/csharpier-compat` reproduces
//! CSharpier's. Every case goes through the shared checks of `common::fmt_preset`.

mod common;

use common::fmt_preset;

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
fn microsoft_preset_is_the_default_layout() {
    let input = "class C : B { int X { get { return 1; } } public C(int a) : base(a) { } }";
    assert_eq!(
        fmt_preset(input, "csharp/microsoft"),
        common::fmt(input, &[])
    );
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
    for name in ["microsoft", "dotnet-runtime", "google", "csharpier-compat"] {
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
    let started = std::time::Instant::now();
    let out = common::fmt(&input, &[]);
    assert!(out.contains("Run(() =>"));
    assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
}
