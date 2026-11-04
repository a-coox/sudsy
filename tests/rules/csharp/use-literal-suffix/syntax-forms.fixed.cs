class A {
 int a, b;
 Nullable<int> n = 1L < 2 ? null : null;
 void F(object o, int i) {
  var f = delegate (int x) { return x; };
  var g = null == o || 1 < i;
  var h = 1F + (double)2.5;
 }
}
