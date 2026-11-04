class A {
 int P { set { } get { return 1; } }
//       ^^^^^^^ accessor-order
 event System.Action E { remove { } add { } }
//                       ^^^^^^ accessor-order
 int Q { set { } /* why */ get { return 1; } }
//       ^^^^^^^ accessor-order
}
