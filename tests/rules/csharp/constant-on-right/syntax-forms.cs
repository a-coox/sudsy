class A {
 int a, b;
 Nullable<int> n = (long)1 < 2 ? null : null;
 void F(object o, int i) {
  var f = delegate (int x) { return x; };
  var g = null == o || 1 < i;
//        ^^^^ constant-on-right
//                     ^ constant-on-right
  var h = (float)1 + (double)2.5;
 }
}
