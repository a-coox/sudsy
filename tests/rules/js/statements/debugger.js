debugger;
^^^^^^^^^ no-debugger
with (a) { b; }
^^^^^^^^^^^^^^^ no-with
var x; delete x; delete x.y; delete (x);
//     ^^^^^^^^ no-delete-var
//                           ^^^^^^^^^^ no-delete-var
switch (a) { case 1: let b = 1; const c = 2; function d() {} class E {} var f = 1; break; default: { let g; } }
//                   ^^^^^^^^^^ no-case-declarations
//                              ^^^^^^^^^^^^ no-case-declarations
//                                           ^^^^^^^^^^^^^^^ no-case-declarations
//                                                           ^^^^^^^^^^ no-case-declarations
new Promise(async (resolve) => resolve()); new Promise(async function () {}); new Promise((resolve) => resolve()); new Promise();
//          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-async-promise-executor
//                                                     ^^^^^^^^^^^^^^^^^^^^ no-async-promise-executor
a.hasOwnProperty("b"); a?.isPrototypeOf(b); a["propertyIsEnumerable"]("c"); Object.prototype.hasOwnProperty.call(a, "b"); a.hasOwnProperty;
//^^^^^^^^^^^^^^ no-prototype-builtins
//                        ^^^^^^^^^^^^^ no-prototype-builtins
//                                            ^^^^^^^^^^^^^^^^^^^^^^ no-prototype-builtins
var NaN, undefined, eval; function arguments() {} try {} catch (Infinity) {}
//  ^^^ no-shadow-restricted-names
//                  ^^^^ no-shadow-restricted-names
//                                 ^^^^^^^^^ no-shadow-restricted-names
//                                                              ^^^^^^^^ no-shadow-restricted-names
