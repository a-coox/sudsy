class A { void F() {
    int k = 5;
//      ^ unused-variable
    int a = 1, b = 2;
    try { F(); } catch (System.Exception e) { }
//               ^^^^^ no-empty-catch
//                                       ^ unused-variable
    System.Console.WriteLine(b + a);
} }
