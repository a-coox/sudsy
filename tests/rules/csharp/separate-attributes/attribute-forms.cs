[assembly: A, B]
//            ^ separate-attributes
class C {
 [A(), B]
//     ^ separate-attributes
 void F([A, B] int p) { }
 [A, // why
  B]
//^ separate-attributes
 int x;
 Action a = delegate() { };
}
