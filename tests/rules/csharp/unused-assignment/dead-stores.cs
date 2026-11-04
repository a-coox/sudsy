class A { int F() { int n = 0; n = G(); return n; } int H() { int m = 0; m = 1; m = G(); return m; } int G() => 1; }
//                      ^^^^^ unused-assignment
//                                                                ^^^^^ unused-assignment
//                                                                       ^^^^^ unused-assignment
