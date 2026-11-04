class A {
 int a, b;
 Nullable<int> n = (long)1 < 2 ? null : null;
 void F(object o, int i) {
  var f = delegate (int x) { return x; };
//        ^^^^^^^^ use-lambda
  var g = null == o || 1 < i;
  var h = (float)1 + (double)2.5;
 }
}
