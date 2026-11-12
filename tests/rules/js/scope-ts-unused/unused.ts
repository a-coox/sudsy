import { Used, Unused, OnlyTyped, Queried } from "m";
//             ^^^^^^ no-unused-vars
import type { TU } from "t";
//            ^^ no-unused-vars
export interface Exported { a: Used }
interface Local { b: string }
//        ^^^^^ no-unused-vars
type Alias = number; type UsedAlias = Alias;
enum E { A, B }
namespace N { export const n = 1 }
const x = 1; type Q = typeof x;
//    ^ no-unused-vars
//                ^ no-unused-vars
function f<T, U>(a: T): T { return a; }
//            ^ no-unused-vars
function over(a: number): void;
function over(a: string): void;
function over(a: any) {}
//            ^ no-unused-vars
class C { constructor(private p: string, q: number) {} m(cb: (z: number) => void) {} }
//                                       ^ no-unused-vars
//                                                       ^^ no-unused-vars
declare function d(a: number): void;
let t: UsedAlias, o: OnlyTyped, qq: typeof Queried;
console.log(f, over, C, E, N, d, t, o, qq);
