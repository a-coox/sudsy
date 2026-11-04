class A {
  int  p = 1;
#if false
  void  M( ) { int  q=1; }
#endif
#if NEVER_DEFINED
  void  N( ) { int  r=1; }
#endif
}
