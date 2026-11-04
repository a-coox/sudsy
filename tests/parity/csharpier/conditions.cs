class A {
#if DEBUG && !TRACE
  void  Debug( ) { int  a=1; }
#endif

#if NET8_0 || NET9_0
  int   Modern  =  1;
#elif NETSTANDARD2_0
  int   Legacy  =  2;
#else
  int   Other  =  3;
#endif

#if !DEBUG
  void  Release( ) { var  x=new  List<int>{1,2,3}; }
#endif
}
