using System.Diagnostics.CodeAnalysis;
class A { [DoesNotReturn] static void Fail() => throw null; void F() { Fail(); F(); } }
//                                                                             ^^^^ unreachable-code
