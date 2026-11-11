function f() { try { a(); } finally { return 1; } }
//                                    ^^^^^^^^^ no-unsafe-finally
function g() { try { a(); } finally { throw e; } }
//                                    ^^^^^^^^ no-unsafe-finally
function h() { for (;;) { try { a(); } finally { break; } } }
//                                               ^^^^^^ no-unsafe-finally
function i() { try { a(); } finally { for (;;) { break; } } }
function j() { try { a(); } finally { function k() { return 1; } } }
function l() { l: try { a(); } finally { break l; } }
//                                       ^^^^^^^^ no-unsafe-finally
function m() { try { a(); } finally { m: { break m; } } }
var o = { get a() {}, get b() { return; }, get c() { return 1; }, get d() { if (x) return 1; }, get e() { throw 1; } };
//        ^^^^^ getter-return
//                              ^^^^^^^ getter-return
//                                                                ^^^^^ getter-return
class K { get a() {} static get b() { return 1 } get c() { while (true) {} } }
//        ^^^^^ getter-return
function* p() {}
function* q() { r(); }
^^^^^^^^^^^^^^^^^^^^^^ require-yield
function* s() { yield 1; }
var t = { *u() { v(); } };
//          ^^^^^^^^^^^ require-yield
class L { *w() { x(); } async *y() { z(); } }
//          ^^^^^^^^^^^ require-yield
//                              ^^^^^^^^^^^ require-yield
