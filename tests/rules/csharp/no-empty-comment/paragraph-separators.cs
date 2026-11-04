class A {
 //no space
 //
 // paragraph
 //
 // text
 //
 ^^ no-empty-comment
 int a; //tail
 /// <summary>x</summary>
 ///<summary>y</summary>
 ///   <summary>z</summary>
 /* */
 ^^^^^ no-empty-comment
 int b;
}
