import { a, b } from "m";
//       ^ no-unused-vars
import c from "c";
export const exported = 1;
const unused = 1, used = 2; let assigned = 1; assigned = 2;
//    ^^^^^^ no-unused-vars
//                                            ^^^^^^^^ no-unused-vars
function f(x, y, z) { return y; }
//       ^ no-unused-vars
//               ^ no-unused-vars
function g(p, q) { return p; }
//       ^ no-unused-vars
//            ^ no-unused-vars
function h({ d }, e = 1, ...rest) {}
//       ^ no-unused-vars
//           ^ no-unused-vars
//                ^ no-unused-vars
//                          ^^^^ no-unused-vars
const fn = function self() { self(); };
function recursive() { recursive(); }
//       ^^^^^^^^^ no-unused-vars
let counter = 0; counter++; counter += 1;
//                          ^^^^^^^ no-unused-vars
let w = 0; w = w + 1; let v; v = () => v;
//         ^ no-unused-vars
//                           ^ no-unused-vars
try {} catch (err) {}
//            ^^^ no-unused-vars
class Cls { static self = Cls; }
//    ^^^ no-unused-vars
const o = { set s(value) {} };
export function loop() { for (const k in o) { return; } }
var dup = 1; var dup = 2;
//               ^^^ no-unused-vars
export const nested = function nested() { var nested = 1; };
//                                            ^^^^^^ no-unused-vars
console.log(used, b, fn, c);
