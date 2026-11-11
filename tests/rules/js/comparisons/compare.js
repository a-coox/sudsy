if (a === NaN || NaN != a || a < Number.NaN || a === NaN2) {}
//  ^^^^^^^^^ use-isnan
//               ^^^^^^^^ use-isnan
//                           ^^^^^^^^^^^^^^ use-isnan
switch (NaN) { case NaN: break; case 1: }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ use-isnan
//             ^^^^^^^^^^^^^^^^ use-isnan
typeof a === "strnig"; typeof a == "object"; "number" !== typeof a; typeof a === undefined; typeof a === typeof b;
//           ^^^^^^^^ valid-typeof
//                                                                               ^^^^^^^^^ valid-typeof
if (!a in b) {} if (!a instanceof B) {} if ((!a) in b) {} if (!(a in b)) {}
//  ^^^^^^^ no-unsafe-negation
//                  ^^^^^^^^^^^^^^^ no-unsafe-negation
if (x === -0) {} if (-0 < x) {} if (x === +0) {} if (x + -0 === y) {}
//  ^^^^^^^^ no-compare-neg-zero
//                   ^^^^^^ no-compare-neg-zero
