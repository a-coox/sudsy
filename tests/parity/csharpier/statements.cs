class A {
  void  M( int  a ) {
#if LOGGING
    Log(  "start"  );
#endif
    if(a>1){
#if CHECKS
      Check(a ,  1);
#else
      Skip(a ,  1);
#endif
    }
    var  y = a+1;
  }
}
