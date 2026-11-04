class A {
 void M() { }
 private int b;
//           ^ member-order
 public int a;
//          ^ member-order
 const int C = 1;
 static int s;
 int i;
 readonly int r;
//            ^ member-order
 static readonly int sr;
//                   ^^ member-order
 public A() { }
}
class B { }
enum E { }
//   ^ member-order
