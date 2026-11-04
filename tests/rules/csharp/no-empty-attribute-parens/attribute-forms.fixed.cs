[assembly: A, B]
class C {
 [A, B]
 void F([A, B] int p) { }
 [A, // why
  B]
 int x;
 Action a = delegate() { };
}
