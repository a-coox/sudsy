//! Statements, declarations and literals against Prettier's output (each expectation was checked
//! with Prettier 3 under its defaults), with every check of the shared helper.

use core_config::OptionValue;
use core_format::testing::assert_formats;
use lang_js::JsProvider;
use provider_api::format_testing::Formatting;

fn fmt(input: &str) -> String {
    assert_formats(&Formatting::new(JsProvider::new(), "a.ts"), input)
}

fn fmt_with(options: Vec<(&'static str, OptionValue)>, input: &str) -> String {
    let mut subject = Formatting::new(JsProvider::new(), "a.ts");
    subject.options = options;
    assert_formats(&subject, input)
}

#[test]
fn statements_and_blank_lines() {
    assert_eq!(
        fmt(
            "'use strict'\nlet a=1,b\nconst c=2,d=3\n\n\n\nif(a)b();else if(c){d()}else e()\nfor(let i=0;i < n;i++)f(i)\nfor(;;){}\nfor(const k in o)g(k)\nasync function q(){for await(const x of y){}}\nwhile(1)x()\ndo{y()}while(z)\nlbl:for(;;)break lbl\n"
        ),
        "\"use strict\";\nlet a = 1,\n  b;\nconst c = 2,\n  d = 3;\n\nif (a) b();\nelse if (c) {\n  d();\n} else e();\nfor (let i = 0; i < n; i++) f(i);\nfor (;;) {}\nfor (const k in o) g(k);\nasync function q() {\n  for await (const x of y) {\n  }\n}\nwhile (1) x();\ndo {\n  y();\n} while (z);\nlbl: for (;;) break lbl;\n"
    );
}

#[test]
fn switch_try_and_jumps() {
    assert_eq!(
        fmt(
            "switch(x){case 1:case 2:a();break\n\ndefault:{b()}}\ntry{f()}catch(e){g()}finally{h()}\ntry{}catch{}\nthrow new Error('x')\nfunction f(){return}\ndebugger\n"
        ),
        "switch (x) {\n  case 1:\n  case 2:\n    a();\n    break;\n\n  default: {\n    b();\n  }\n}\ntry {\n  f();\n} catch (e) {\n  g();\n} finally {\n  h();\n}\ntry {\n} catch {}\nthrow new Error(\"x\");\nfunction f() {\n  return;\n}\ndebugger;\n"
    );
}

#[test]
fn functions_classes_and_modules() {
    assert_eq!(
        fmt(
            "export default async function*  f<T>(a: T, b: number = 1, ...r): void {}\nfunction g(this: X, { a, b }){}\nexport class A<T> extends B implements C,D{\nstatic x?: number = 1\n\n\n  async *m(a){}\nget [k](){return 1}\n@dec() p;constructor(private a: string){super()}\nabstract n(): void\nstatic{init()}\n}\nimport d,{a as b,type c} from 'x' with { type: 'json' }\nimport {} from 'y'\nimport 'z'\nexport {a,b as c}\nexport * as ns from \"z\"\n"
        ),
        "export default async function* f<T>(a: T, b: number = 1, ...r): void {}\nfunction g(this: X, { a, b }) {}\nexport class A<T> extends B implements C, D {\n  static x?: number = 1;\n\n  async *m(a) {}\n  get [k]() {\n    return 1;\n  }\n  @dec() p;\n  constructor(private a: string) {\n    super();\n  }\n  abstract n(): void;\n  static {\n    init();\n  }\n}\nimport d, { a as b, type c } from \"x\" with { type: \"json\" };\nimport {} from \"y\";\nimport \"z\";\nexport { a, b as c };\nexport * as ns from \"z\";\n"
    );
}

#[test]
fn long_parameters_and_import_lists_break() {
    assert_eq!(
        fmt(
            "function longName(parameterNumberOne, parameterNumberTwo, parameterNumberThree, four) {}\nimport {aaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, cccccccccccccccccc, ddddddddddddddddd} from 'x'\n"
        ),
        "function longName(\n  parameterNumberOne,\n  parameterNumberTwo,\n  parameterNumberThree,\n  four,\n) {}\nimport {\n  aaaaaaaaaaaaaaaa,\n  bbbbbbbbbbbbbbbbbbb,\n  cccccccccccccccccc,\n  ddddddddddddddddd,\n} from \"x\";\n"
    );
}

#[test]
fn literals_are_restyled_inside_unformatted_expressions() {
    assert_eq!(
        fmt("f('a', \"it's\", 'say \"hi\"', 0XAB, 1.50, .5, 5., 1E+05, /x/gi, /y/ig, 10n)\n"),
        "f(\"a\", \"it's\", 'say \"hi\"', 0xab, 1.5, 0.5, 5, 1e5, /x/gi, /y/gi, 10n);\n"
    );
}

#[test]
fn comments_stay_with_their_tokens() {
    assert_eq!(
        fmt(
            "// top\nlet a = 1; // one\n/* two */ if (a) {\n  // three\n  b(); // four\n}\n// last\n"
        ),
        "// top\nlet a = 1; // one\n/* two */ if (a) {\n  // three\n  b(); // four\n}\n// last\n"
    );
}

#[test]
fn the_semi_and_quote_options() {
    let semi_off = vec![("semi", OptionValue::Bool(false))];
    assert_eq!(
        fmt_with(
            semi_off,
            "const a = 1;\n[1].map(f);\n(g)();\nclass A { x = 1; [k] = 2; y; *m() {} }\nfor (;;) {}\n"
        ),
        "const a = 1\n;[1].map(f)\ng()\nclass A {\n  x = 1;\n  [k] = 2\n  y;\n  *m() {}\n}\nfor (;;) {}\n"
    );
    let single = vec![("quote_style", OptionValue::String("single".into()))];
    assert_eq!(
        fmt_with(
            single,
            "'use strict'\nf(\"a\", 'b', \"it's\", \"say \\\"hi\\\"\")\nimport x from \"x\"\n"
        ),
        "'use strict';\nf('a', 'b', \"it's\", 'say \"hi\"');\nimport x from 'x';\n"
    );
}

#[test]
fn empty_statements_in_a_list_go() {
    assert_eq!(
        fmt("function f() {};\nif (a) {\n  ;b.c()\n  ;\n} else ;\nfor (;;) ;\n"),
        "function f() {}\nif (a) {\n  b.c();\n} else;\nfor (;;);\n"
    );
}

#[test]
fn class_semicolons_and_specifier_blank_lines() {
    assert_eq!(
        fmt("class A { m() {}; x = 1; }\nexport {\n  // a\n  a,\n\n  // b\n  b,\n}\n"),
        "class A {\n  m() {}\n  x = 1;\n}\nexport {\n  // a\n  a,\n\n  // b\n  b,\n};\n"
    );
}

#[test]
fn prettier_ignore_keeps_a_statement() {
    assert_eq!(
        fmt("// prettier-ignore\nlet   a  =  'x'\nlet   b  =  'y'\n"),
        "// prettier-ignore\nlet   a  =  'x'\nlet b = \"y\";\n"
    );
}

#[test]
fn typescript_types_and_declarations() {
    assert_eq!(
        fmt(
            r#"type A = "aaaaaaaaaaaaaaaaaaaaaaa" | "bbbbbbbbbbbbbbbbbbbbbbbbbbb" | "cccccccccccccccccccccc" | "dddddddddd";
type B<T> = (T extends string ? "a" : T extends number ? "b" : "cccccccccccccccccccccccccccccccccccccccccccc");
type C = { readonly [K in keyof T as `get${K}`]-?: T[K] } & { a: string, b?: number };
interface D<T extends object = {}> extends E, F { a: string; readonly b?: number, [key: string]: any; m<U>(x: U): void; new (x: number): D<T> }
enum G { A = 1, B, C = "c" }
declare module "m" { export function f(a: (string | number)[], b: Array<(() => void)>): void; }
namespace H.I { export const x = 1 }
abstract class J<T> extends K<T> implements L, M { abstract m(): void; constructor(private a: string, readonly b = 1) { super() } }
function f<T extends keyof U, U>(this: Window, a: T, ...rest: U[]): asserts a is T {}
let v = x as unknown as B, w = <T>y!, z = a satisfies B;
type N = [a: string, b?: number, ...rest: boolean[]];
type O = (aaaaaaaaaaaaaaaaaaaaaaa: string, bbbbbbbbbbbbbbbbbbbbbbbbbbb: number, ccccccccccccccc: boolean) => void;
"#
        ),
        r#"type A =
  | "aaaaaaaaaaaaaaaaaaaaaaa"
  | "bbbbbbbbbbbbbbbbbbbbbbbbbbb"
  | "cccccccccccccccccccccc"
  | "dddddddddd";
type B<T> = T extends string
  ? "a"
  : T extends number
    ? "b"
    : "cccccccccccccccccccccccccccccccccccccccccccc";
type C = { readonly [K in keyof T as `get${K}`]-?: T[K] } & {
  a: string;
  b?: number;
};
interface D<T extends object = {}> extends E, F {
  a: string;
  readonly b?: number;
  [key: string]: any;
  m<U>(x: U): void;
  new (x: number): D<T>;
}
enum G {
  A = 1,
  B,
  C = "c",
}
declare module "m" {
  export function f(a: (string | number)[], b: Array<() => void>): void;
}
namespace H.I {
  export const x = 1;
}
abstract class J<T> extends K<T> implements L, M {
  abstract m(): void;
  constructor(
    private a: string,
    readonly b = 1,
  ) {
    super();
  }
}
function f<T extends keyof U, U>(
  this: Window,
  a: T,
  ...rest: U[]
): asserts a is T {}
let v = x as unknown as B,
  w = <T>y!,
  z = a satisfies B;
type N = [a: string, b?: number, ...rest: boolean[]];
type O = (
  aaaaaaaaaaaaaaaaaaaaaaa: string,
  bbbbbbbbbbbbbbbbbbbbbbbbbbb: number,
  ccccccccccccccc: boolean,
) => void;
"#
    );
}
