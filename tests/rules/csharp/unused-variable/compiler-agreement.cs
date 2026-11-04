using System;
class A {
  int F(int a) {
    int unused;
//      ^^^^^^ unused-variable
    int k = 5;
//      ^ unused-variable
    int z = G();
    try { G(); } catch (Exception e) { }
//               ^^^^^ no-empty-catch
//                                ^ unused-variable
    if (false) { G(); }
    while (true) { if (a > 0) break; }
    return z;
    G();
    G();
  }
  int G() => 1;
  void Quit() { Environment.Exit(1); G(); }
  void Lambda() { int c = 1; Action f = () => Console.WriteLine(c); f(); }
}
