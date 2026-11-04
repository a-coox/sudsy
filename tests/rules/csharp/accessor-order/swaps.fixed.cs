class A {
 int P { get { return 1; } set { } }
 event System.Action E { add { } remove { } }
 int Q { set { } /* why */ get { return 1; } }
}
