import d, { n as m } from "m";
import * as ns from "ns";
const c = 1; c = 2; c += 1; c++; [c] = [1]; ({ c } = {}); for (c of []) {}
//           ^ no-const-assign
//                  ^ no-const-assign
//                          ^ no-const-assign
//                                ^ no-const-assign
//                                             ^ no-const-assign
//                                                             ^ no-const-assign
class K { static f() { K = 1; } } K = 2; (class L { f() { L = 1; } });
//                     ^ no-class-assign
//                                ^ no-class-assign
//                                                        ^ no-class-assign
function g() {} g = 3; (function h() { h = 1; });
//              ^ no-func-assign
//                                     ^ no-func-assign
d = 1; m++; ns = 1; ns.x = 1; delete ns.y; ns.a.b = 1; [ns.z] = []; Object.assign(ns, {}); Object.keys(ns);
^ no-import-assign
//     ^ no-import-assign
//          ^ no-import-assign
//                  ^ no-import-assign
//                            ^ no-import-assign
//                                                     ^ no-import-assign
//                                                                  ^ no-import-assign
const o = { c }; const [q] = [c]; var v = 1; v = 2; let l = 1; l = 2;
