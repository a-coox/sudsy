function a() { return 1; b(); var v; c(); function h() {} }
//                       ^^^^ no-unreachable
//                                   ^^^^ no-unreachable
function b() { throw e; ; d(); }
//                        ^^^^ no-unreachable
function c(x) { if (x) { return; } else { return; } e(); }
//                                                  ^^^^ no-unreachable
function d() { while (true) {} f(); }
//                             ^^^^ no-unreachable
function e(x) { for (;;) { if (x) break; } g(); }
function f(x) { switch (x) { case 1: return; default: return; } h(); }
//                                                              ^^^^ no-unreachable
function g(x) { switch (x) { case 1: return; } h(); }
function h(x) { try { return 1; } finally { i(); } j(); }
//                                                 ^^^^ no-unreachable
function i(x) { l: { break l; } k(); }
function j() { for (;;) { continue; k(); } }
//                                  ^^^^ no-unreachable
function k(x) { try { return 1; } catch (e) { } m(); }
//                                          ^^^ no-unreachable
function l(x) { do { break; } while (x); n(); }
function m() { return; let z = 1; const y = 2; class Z {} }
//                     ^^^^^^^^^^ no-unreachable
