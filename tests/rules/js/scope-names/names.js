use(undefinedThing, typeof maybe, typeof maybe.x, arguments);
^^^ no-undef
//  ^^^^^^^^^^^^^^ no-undef
//                                       ^^^^^ no-undef
//                                                ^^^^^^^^^ no-undef
function f(a, a2) { var a; return arguments; }
//                      ^ no-redeclare
var x = 1; var x = 2; function x() {}
//             ^ no-redeclare
//                             ^ no-redeclare
function f2() {} function f2() {}
//                        ^^ no-redeclare
try {} catch (e) { var e; }
before(); function before() {} before2(); var before2 = 1;
^^^^^^ no-use-before-define
//                             ^^^^^^^ no-use-before-define
const s = s + 1; let t = () => t; class C { static p = C; q = C; } class D extends D {}
//        ^ no-use-before-define
//                                                                                 ^ no-use-before-define
for (var i of i) {} const { u = u } = {};
//            ^ no-use-before-define
//                              ^ no-use-before-define
Promise.all([globalThis, hasOwnProperty]);
