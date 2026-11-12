interface I { a: 1 } interface I { b: 2 }
class K {} interface K {}
namespace M {} namespace M {}
function fo(): void; function fo() {}
enum En {} namespace En {}
type T1 = 1; const T1 = 2;
//                 ^^ no-redeclare
var v = 1; var v = 2;
//             ^ no-redeclare
class Two {} class Two {}
//                 ^^^ no-redeclare
type T = string; function f<T>(a: T) {}
//                          ^ no-shadow
const a = 1; type Cb = (a: number) => void; interface S { m(a: string): void }
class G<X> { static m<X>() {} }
const tv = 1; function g() { type tv = number; }
let l: Later; type Later = number; const b: typeof c = 1; const c = 1;
enum E1 { A = E2.X } enum E2 { X }
//            ^^ no-use-before-define
function u() { return Hoisted; } const Hoisted = 1;
//                    ^^^^^^^ no-use-before-define
