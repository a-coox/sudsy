class A { void F() {
    
    int a = 1, b = 2;
    try { F(); } catch (System.Exception) { }
    System.Console.WriteLine(b + a);
} }
