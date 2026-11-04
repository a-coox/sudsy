//! Formatter snapshots: one case per construct, per hook and per option value. Every case goes
//! through the shared checks (lossless round-trip, unchanged meaning, idempotency, comments kept)
//! via `common::fmt`; the snapshot shows the input and the output.

mod common;

use common::{fmt, l, n, s, subject};
use core_config::OptionValue;
use core_format::testing::assert_formats;

macro_rules! snap {
    ($name:ident, $input:expr) => {
        snap!($name, $input, []);
    };
    ($name:ident, $input:expr, [$(($k:expr, $v:expr)),* $(,)?]) => {
        #[test]
        fn $name() {
            let input: &str = $input;
            let out = fmt(input, &[$(($k, $v)),*]);
            insta::assert_snapshot!(format!("{input}\n-----\n{out}"));
        }
    };
}

// Declarations -----------------------------------------------------------------------------------

snap!(
    class_with_members,
    "using System;\nnamespace N{public class C:Base,IFoo{private int x=1;public int X{get;set;}\npublic C(int x):base(x){this.x=x;}\npublic void M(){}}}"
);
snap!(
    file_scoped_namespace,
    "namespace N;\nusing System;\n\n\npublic class C { }\nclass D { int x; }"
);
snap!(
    properties_and_accessors,
    "class C {\n int A { get; set; }\n int B { get; init; } = 3;\n int C2 { get { return 1; } set { x = value; } }\n int D => 4;\n public int E\n {\n get;\n set;\n }\n}"
);
snap!(
    enums_and_delegates,
    "enum E{A,B=2,C}\nenum F : byte { X = 1, Y, }\ndelegate void D(int x);\n[Flags] enum G { None = 0, All = 1 | 2 }"
);
snap!(
    generics_and_constraints,
    "class C<T,U>where T:class,new()where U:struct{ void M<V>(V v)where V:IComparable<V>{} Dictionary<string,List<int>> d; }"
);
snap!(
    records_and_primary_constructors,
    "public record Person(string First,string Last);\nrecord struct P(int X,int Y){public int Sum=>X+Y;}\nclass C(int a,string b):Base(a){int f=a;}\npublic record R(int A){public int B{get;init;}}"
);
snap!(
    interface_and_operators,
    "interface I{void M();int P{get;}}\nstruct S{public static S operator+(S a,S b)=>a;public static implicit operator int(S s)=>0;}"
);
snap!(
    empty_bodies_stay_compact,
    "class C { }\nclass D\n{\n}\nclass E { void M() { } void N()\n{\n}\n}"
);

// Statements ---------------------------------------------------------------------------------------

snap!(
    control_flow,
    "class C{void M(int a){if(a>1)return;else if(a<0){x();}else y();\nwhile(a>0)a--;\ndo{a++;}while(a<5);\nfor(int i=0;i<10;i++)Foo(i);\nfor(;;){break;}\nforeach(var x in xs)Bar(x);\nlock(o){x();}\nusing(var f=Open())f.Read();\nusing var g=Open();\n}}"
);
snap!(
    try_catch_finally,
    "class C{void M(){try{a();}catch(IOException e)when(e.Code==1){b();}catch{c();}finally{d();}}}"
);
snap!(
    switch_statement,
    "class C{int M(int x){switch(x){case 1:\ncase 2:return 1;\ncase 3:{return 2;}\ndefault:return 0;}}}"
);
snap!(
    switch_expression,
    "class C{int M(int x)=>x switch{1=>10,2=>20,_=>0};\nstring N(object o)=>o switch{int i when i>0=>\"pos\",string{Length:>3}=>\"long\",null=>\"null\",_=>\"other\",};}"
);
snap!(
    local_functions_and_labels,
    "class C{void M(){int Local(int x)=>x+1;\nasync Task<int> L2(){await Task.Delay(1);return 1;}\nlabel:\nx();goto label;}}"
);
snap!(
    top_level_statements,
    "using System;\nvar x=1;\nConsole.WriteLine(x);\nif(x>0){Console.WriteLine(\"p\");}\nclass C{}"
);

// Expressions ----------------------------------------------------------------------------------------

snap!(
    unary_and_postfix,
    "class C{void M(){var a=-x;var b=- -x;var c=!y;var d=~z;var e=x++ + ++x;var f=x-(-y);var g=(int)-x;var h=a!;var i=^1;var j=a..b;}}"
);
snap!(
    object_and_collection_initializers,
    "class C{void M(){var a=new Foo{A=1,B=2};var b=new Foo(1){A=1};var c=new List<int>{1,2,3};var d=new Dictionary<string,int>{{\"a\",1},{\"b\",2}};var e=new[]{1,2,3};var f=new int[3];var g=new {A=1,B=\"x\"};var h=new Foo(){};Foo i=new();}}"
);
snap!(
    initializer_that_was_multiline_stays_multiline,
    "class C{void M(){var a=new Foo\n{\nA=1,\nB=2\n};\nvar b=new List<int>{\n1,\n2};}}"
);
snap!(
    collection_expressions,
    "class C{void M(){int[] a=[1,2,3];List<int> b=[..a,4];var c=[];int[][] d=[[1],[2,3]];}}"
);
snap!(
    long_collection_expression_breaks,
    "class C{int[] a=[1000000,2000000,3000000,4000000,5000000,6000000,7000000,8000000,9000000,10000000,11000000,12000000];}"
);
snap!(
    tuples_and_deconstruction,
    "class C{void M(){var (a,b)=Foo();(int x,int y)=(1,2);var t=(a:1,b:2);(string,int) u=(\"a\",1);}}"
);
snap!(
    casts_and_conversions,
    "class C{void M(){var a=(int)x;var b=(Foo)(bar);var c=x as Foo;var d=x is Foo f&&f.Ok;var e=(a)(b);var f=typeof(int);var g=default(int);var h=nameof(C);}}"
);
snap!(
    conditional_and_coalesce,
    "class C{void M(){var a=x?y:z;var b=x??y??z;a??=b;var c=x?.y?.z;var d=x?[0];var e=a?b:c?d:e;}}"
);
snap!(
    conditional_breaks_when_long,
    "class C{void M(){var result=someCondition&&anotherCondition?computeTheFirstValue(argumentNumberOne,argumentNumberTwo):computeTheSecondValue(argumentNumberOne);}}"
);
snap!(
    patterns,
    "class C{bool M(object o){if(o is int i&&i>0)return true;\nif(o is not null)return false;\nif(o is string{Length:>3 or <1}s)return true;\nif(o is(1,2))return true;\nif(o is [1,2,..])return true;\nif(o is Point{X:0,Y:0})return true;\nreturn o is { };}}"
);
snap!(
    lambdas,
    "class C{void M(){Func<int,int> a=x=>x+1;Func<int,int,int> b=(x,y)=>x+y;Action c=()=>{Foo();};var d=async()=>await Bar();var e=static(int x)=>x;Func<int> f=delegate(){return 1;};}}"
);
snap!(
    attributes,
    "[assembly:InternalsVisibleTo(\"X\")]\n[Serializable][Obsolete(\"x\",true)]\nclass C{[Fact]public void M([FromBody]int x,[A,B]string y){}\n[return:NotNull]public int N()=>1;\n[A(1,B=2,C=\"x\")]int f;}"
);
snap!(
    long_attribute_list_breaks,
    "[Attribute1(\"a long argument\"), Attribute2(\"another long argument\"), Attribute3(\"and one more long argument\"), Attr4]\nclass C{}"
);

// Hooks ---------------------------------------------------------------------------------------------------

snap!(
    member_chain_stays_on_one_line_when_it_fits,
    "class C{void M(){var a=items.Where(x=>x>1).Select(x=>x*2).ToList();}}"
);
snap!(
    member_chain_breaks_when_it_does_not_fit,
    "class C{void M(){var a=repository.GetAllCustomers().Where(customer=>customer.IsActive&&customer.Orders.Count>0).OrderBy(customer=>customer.LastName).Select(customer=>customer.Name).ToList();}}"
);
snap!(
    member_chain_with_this_and_static_head,
    "class C{void M(){this.builder.WithName(\"a long name here\").WithDescription(\"another long text here\").WithOwner(owner).Build();\nvar x=Enumerable.Range(0,1000000).Where(i=>i%2==0).Select(i=>i*i).Where(i=>i>100).ToArray().Length;}}"
);
snap!(
    member_chain_fewer_calls_than_minimum_does_not_break,
    "class C{void M(){var a=someObject.SomeMethodWithALongName(argumentNumberOne,argumentNumberTwo).AnotherMethodWithALongName(argumentNumberThree,argumentNumberFour);}}"
);
snap!(
    member_chain_min_calls_option,
    "class C{void M(){var a=someObject.SomeMethodWithALongName(argumentNumberOne,argumentNumberTwo).AnotherMethodWithALongName(argumentNumberThree,argumentNumberFour);}}",
    [("member_chain_min_calls", n(2))]
);
snap!(
    conditional_access_chains,
    "class C{void M(){var a=customer?.Address?.Street?.Trim().ToUpper().Substring(0,3).PadLeft(10).Replace(\"a\",\"b\")?.Length;a!.B!.C();}}"
);
snap!(
    lambda_as_last_argument_hugs_the_call,
    "class C{void M(){list.ForEach(x=>{Console.WriteLine(x);Foo(x);});\nTask.Run(async()=>{await Foo();});\nservices.AddSingleton<IFoo>(provider=>{return new Foo(provider);});\nTest(\"name\",first,second,(a,b)=>{return a+b;});}}"
);
snap!(
    lambda_hug_falls_back_when_the_arguments_do_not_fit,
    "class C{void M(){RegisterTheThing(\"a very long first argument that takes room\",\"a second very long argument that takes more room\",\"third\",x=>{Foo();});}}"
);
snap!(
    nested_lambdas,
    "class C{void M(){a.Foo(x=>{b.Bar(y=>{c.Baz(z=>{return z;});return y;});return x;});}}"
);
snap!(
    long_argument_list_breaks_one_per_line,
    "class C{void M(){Foo(argumentNumberOne,argumentNumberTwo,argumentNumberThree,argumentNumberFour,argumentNumberFive,six);}\nvoid LongParameters(int parameterNumberOne,string parameterNumberTwo,double parameterNumberThree,object four){}}"
);
snap!(
    binary_expression_trees,
    "class C{bool M(){return aaaaaaaaaaaaaaaaaaaaaaaa!=null&&bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>100||cccccccccccccccccccccccccc.Length>1000&&ddddddddddddddddddddddd<3&&eeeeeeeeeeeeeeeeeeee!=12345;}}"
);
snap!(
    binary_in_if_condition_and_assignment,
    "class C{void M(){if(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa!=null&&bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb>100&&cccccccccccccccccccccc.Length>1000){x();}\nvar total=firstValueInTheExpression+secondValueInTheExpression*thirdValueInTheExpression-fourthValue+fifth;\nx=firstValueInTheExpression+secondValueInTheExpression+thirdValueInTheExpression+fourthValueInTheExpression;}}"
);
snap!(
    string_concatenation_breaks,
    "class C{string M()=>\"a long string literal number one \"+name+\" a long string literal number two \"+other+\" and a third long one\";}"
);
snap!(
    linq_query_expressions,
    "class C{void M(){var a=from x in xs where x>1 select x;\nvar b=from customer in customers join order in orders on customer.Id equals order.CustomerId where order.Total>100 orderby customer.Name,order.Total descending select new{customer.Name,order.Total};\nvar c=from x in xs group x by x.Key into g select g.Count();}}"
);

// Comments, directives and verbatim text ---------------------------------------------------------------------

snap!(
    comments_in_odd_places,
    "class C /* a */ {\n// leading\nint x /* b */ = /* c */ 1; // trailing\nvoid M(/* none */) // after paren\n{\n  Foo(1, // first\n      2 /* second */);\n  var a = b // end of line\n      + c;\n  if (x) // cond\n    y();\n  // last in block\n}\n// last in class\n}\n// end of file"
);
snap!(
    comments_inside_lists_and_chains,
    "class C{void M(){var a=new[]{1, // one\n2, /* two */ 3 // three\n};\nx.A() // first\n .B() // second\n .C();\nFoo(/* a */ 1, /* b */ 2);}}"
);
snap!(
    doc_comments_are_reindented,
    "namespace N{\n        /// <summary>\n        ///   A type.\n        /// </summary>\n   public class C{\n        /// <summary>M</summary>\n        /// <param name=\"x\">x</param>\n    public void M(int x){}\n    /** block doc */\n    public void N(){}}}"
);
snap!(
    directives_stay_at_column_zero,
    "class C{\n#region Fields\nint x;\n#endregion\n#if DEBUG\nvoid Debug(){}\n#else\nvoid Release(){}\n#endif\nvoid M(){\n#pragma warning disable CS0168\nint unused;\n#pragma warning restore CS0168\n}}"
);
snap!(
    disabled_code_is_kept_verbatim,
    "class C{\n#if NEVER\n   weird   text   {{{ not   valid\n   int   x;\n#endif\nint   y;}"
);
snap!(
    interpolated_and_raw_strings,
    "class C{void M(){var a=$\"x{y}z {a+b,5:N2}\";var b=$@\"{x}\";var c=@\"verbatim\n  text\";var d=\"\"\"raw \"quoted\" text\"\"\";var e=$$\"\"\"{{x}}\"\"\";var f=\"bytes\"u8;}}"
);
snap!(
    crlf_input_keeps_crlf,
    "class C\r\n{\r\n    void M() { x(); }\r\n}\r\n"
);
snap!(
    blank_lines_are_limited_to_one,
    "class C{\n\n\n int a;\n\n\n\n int b;\n void M(){\n\n a();\n\n\n b();\n\n}\n\n}"
);
snap!(
    ignore_comment_keeps_the_next_statement,
    "class C{void M(){\n// sudsy-ignore\nvar   a  =   1 ;\nvar   b  =   2 ;\n}}"
);
snap!(
    unicode_identifiers_and_text,
    "class C{void M(){var größe=\"日本語\";var ключ=größe+\"😀\";}}"
);

// Options -------------------------------------------------------------------------------------------------------------------

snap!(
    brace_newline_none,
    "class C{void M(){if(a){x();}else{y();}foreach(var i in l){z();}}\nint P{get{return 1;}}}",
    [("brace_newline", s("none"))]
);
const ELEMENTS: &str = "namespace N{class C{int P{get{return 1;}set{x=value;}}\nvoid M(){if(a){b();}else{c();}try{d();}catch(E){e();}finally{f();}\nvar g=delegate(int i){return i;};var h=()=>{j();};\nint Local(){return 1;}switch(x){case 1:break;}\nvar o=new C{\nA=1\n};var p=new{\nA=1\n};}}}";
snap!(
    brace_same_line_types_and_methods,
    ELEMENTS,
    [("brace_same_line", l(&["types", "methods"]))]
);
snap!(
    brace_same_line_blocks_and_lambdas,
    ELEMENTS,
    [(
        "brace_same_line",
        l(&["control_blocks", "lambdas", "anonymous_methods"])
    )]
);
snap!(
    brace_same_line_properties_and_initializers,
    ELEMENTS,
    [(
        "brace_same_line",
        l(&[
            "properties",
            "accessors",
            "object_collection_array_initializers",
            "anonymous_types"
        ])
    )]
);
snap!(
    clause_lines_per_keyword,
    "class C{void M(){if(a){b();}else{c();}try{d();}catch(E){e();}finally{f();}}}",
    [
        ("else_line", s("same_line")),
        ("finally_line", s("same_line"))
    ]
);
snap!(
    clause_line_inherited_unless_set,
    "class C{void M(){if(a){b();}else{c();}try{d();}catch(E){e();}finally{f();}}}",
    [
        ("clause_keyword_line", s("same_line")),
        ("catch_line", s("new_line"))
    ]
);
const LABELS: &str = "class C{void M(){if(a){\nlbl:\nx++;goto lbl;}\nend:\nreturn;}}";
snap!(
    label_indent_one_less,
    LABELS,
    [("label_indent", s("one_less"))]
);
snap!(
    label_indent_flush_left,
    LABELS,
    [("label_indent", s("flush_left"))]
);
snap!(
    trailing_commas_never,
    "enum E{A,B,}\nclass C{int[] a=new[]{1,2,};void M(){var x=y switch{1=>2,_=>3,};}}",
    [("trailing_commas", s("never"))]
);
snap!(
    trailing_commas_multiline,
    "enum E{A,B}\nclass C{int[] a=new[]{1,2};int[] b=new[]\n{\n1,\n2\n};}",
    [("trailing_commas", s("multiline"))]
);
snap!(
    line_width_60,
    "class C{void M(){var result=Compute(firstArgument,secondArgument,thirdArgument)+other;}}",
    [("line_width", n(60))]
);
snap!(
    indent_size_2,
    "class C{void M(){if(a){b();}}}",
    [("indent_size", n(2))]
);
snap!(
    indent_tabs,
    "class C{void M(){if(a){b();}}}",
    [("indent_style", s("tab"))]
);

#[test]
fn symbols_select_the_active_branch() {
    let input =
        "class C\n{\n#if FEATURE\n    void   A() { }\n#else\n    void   B() { }\n#endif\n}\n";
    let on = subject(&[]).with_symbols(&["FEATURE"]);
    let off = subject(&[]);
    let out_on = assert_formats(&on, input);
    let out_off = assert_formats(&off, input);
    assert!(out_on.contains("void A()") && out_on.contains("void   B()"));
    assert!(out_off.contains("void B()") && out_off.contains("void   A()"));
}

#[test]
fn syntax_errors_are_reported_and_nothing_is_formatted() {
    use core_format::testing::Subject;
    let err = subject(&[]).format("class C { void M( { }").unwrap_err();
    assert!(err.contains(':'), "{err}");
}

#[test]
fn every_fixture_formats() {
    let dir = common::repo_root().join("tests/fixtures/csharp");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.extension().is_none_or(|e| e != "cs") || name.starts_with("error_") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let sub = subject(&[]).with_symbols(&["FEATURE", "DEBUG"]);
        if let Err(e) = core_format::testing::check(&sub, &text) {
            panic!("{name}: {e}");
        }
        count += 1;
    }
    assert!(count >= 14, "{count}");
}

#[test]
fn end_of_line_option() {
    let crlf = "class C\r\n{\r\n    int x;\r\n}\r\n";
    assert_eq!(fmt(crlf, &[]), crlf);
    assert_eq!(
        fmt(crlf, &[("end_of_line", s("lf"))]),
        "class C\n{\n    int x;\n}\n"
    );
    assert_eq!(
        fmt("class C { int x; }\n", &[("end_of_line", s("crlf"))]),
        crlf
    );
}

// StyleCop layout options ---------------------------------------------------------------------------------------------

snap!(
    empty_braces_expanded,
    "class C { }\nenum E { }\nclass D { void M() { } int P { get { } } void N() { if (x) { } else { } while (x) { } var a = new A { }; Action f = () => { }; Action g = delegate { }; } }",
    [("empty_braces", s("expanded"))]
);
snap!(
    empty_braces_expanded_without_brace_newline,
    "class C { void M() { } }",
    [
        ("empty_braces", s("expanded")),
        ("brace_newline", s("none"))
    ]
);
snap!(
    closing_paren_same_line,
    "class C { void Method(int firstParameter, string secondParameter, double thirdParameter, bool fourth) { Call(firstArgument, secondArgument, thirdArgument, fourthArgumentIsLong, fifth); Call(a => { b(); }); } [Attr(\"a very long first argument\", \"a very long second argument\", \"third\", \"and fourth\")] int F; int G(\n// c\nint a\n) => 1; }",
    [
        ("closing_paren_line", s("same_line")),
        ("line_width", n(60))
    ]
);
snap!(
    closing_paren_own_line_is_the_default,
    "class C { void Method(int firstParameter, string secondParameter, double thirdParameter, bool fourth) { } }",
    [("line_width", n(60))]
);
snap!(
    blank_lines_between_members_always,
    "using A;\nusing B;\nnamespace N { class C { int a; int b; [X] int c; event Action E; event Action F; int P { get; } int Q { get; set; }\n/// <summary>d</summary>\nvoid M() { } void N() { } }\nenum E { A, B,\n// c\nC } }",
    [("blank_lines_between_members", s("always"))]
);
snap!(
    blank_lines_after_block_required,
    "class C { void M() { if (a) { b(); } c(); { d(); } e(); do { f(); } while (g); h(); switch (x) { case 1: { y(); break; } case 2: break; }\nvoid L() { } l();\n#if X\nz();\n#endif\n} void N() { } int P { get { return 1; } } int Q => 2; }",
    [("blank_lines_after_block", s("require"))]
);
snap!(
    blank_line_before_comment_required,
    "class C { void M() { a();\n// one\n// two\nb();\n/* block */\nc();\n//// code\nd();\nif (x) {\n// first in block\ne(); } } }",
    [("blank_line_before_comment", s("require"))]
);
snap!(
    blank_line_after_comment_removed,
    "// header\n\nclass C {\n/// <summary>s</summary>\n\nvoid M() {\n// one\n\n// two\n\na();\n//// code\n\nb();\n/* block */\n\nc(); } }",
    [("blank_line_after_comment", s("remove"))]
);
snap!(
    delegate_without_space,
    "class C { void M() { Action<int> a = delegate (int v) { return; }; Action b = delegate { }; } }",
    [("space_after_delegate", OptionValue::Bool(false))]
);
snap!(
    for_semicolons,
    "class C { void M() { for (;;) { } for (int i = 0;;) { } for (; i < 3;) { } for (int i = 0; i < 3; i++) { } } }"
);
snap!(
    for_semicolons_spaced,
    "class C { void M() { for (;;) { } for (int i = 0;;) { } for (; i < 3;) { } } }",
    [("space_between_empty_for_clauses", OptionValue::Bool(true))]
);

// Sorting usings ---------------------------------------------------------------------------------

snap!(
    sort_usings_merges_and_sorts,
    "// header

using Zeta;
using static System.Math;
using X = Y.Z;
using System.Linq; // linq
using System;

using Beta;
// about Alpha
using alpha;
using global::Foo;
namespace N
{
    using Zed;

    using Abc;
    class C { }
}
class D { }
",
    [("sort_usings", OptionValue::Bool(true))]
);
snap!(
    sort_usings_leaves_directive_lists,
    "using Zeta;
#if X
using Alpha;
#endif
class C { }
",
    [("sort_usings", OptionValue::Bool(true))]
);
snap!(
    sort_usings_file_scoped_namespace,
    "using Zeta;
using Alpha;
namespace N;
using Beta;
using Aa;
class C { }
",
    [("sort_usings", OptionValue::Bool(true))]
);
