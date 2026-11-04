class A { int F() { Missing(); return 1; throw null; } int G() { Missing(); return 1; ; } int H() { Missing(); return 1; Missing(); } }
//                                                                                                                       ^^^^^^^^^^ unreachable-code
