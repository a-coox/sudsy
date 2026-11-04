class A { int F() { return 1; G(); G(); }
//                            ^^^^ unreachable-code
 int H(int x) { if (false) { G(); } throw null; x++; } int G() => 1; }
//                           ^^^^ unreachable-code
//                                              ^^^^ unreachable-code
