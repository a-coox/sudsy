#nullable enable
class A { string? s; int N() => s.Length; int M() => s!.Length; int P() { if (s != null) return s.Length; return 0; } }
