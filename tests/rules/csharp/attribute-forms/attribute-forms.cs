[assembly: A, B]
//            ^ separate-attributes
class C {
 [A(), B]
// ^ no-empty-attribute-parens
//     ^ separate-attributes
 void F([A, B] int p) { }
 [A, // why
  B]
//^ separate-attributes
 int x;
 Action a = delegate() { };
//                  ^ no-empty-delegate-parens
}
