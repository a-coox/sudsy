using System;
using System.Diagnostics;
namespace A.B {
  using C.D;
//^^^^^^^^^^ qualify-namespace-usings
  struct S { }
  interface I { (int, int) Pair(); }
//               ^^^ tuple-element-names-in-signatures
//                    ^^^ tuple-element-names-in-signatures
  class P { public virtual void Run() { } }
  class Q : P, I {
    public (int, int) Pair() => (1, 2);
    public (int, int) Other() => (1, 2);
//          ^^^ tuple-element-names-in-signatures
//               ^^^ tuple-element-names-in-signatures
    public void Go(ValueTuple<int, int> t) {
//                 ^^^^^^^^^^^^^^^^^^^^ use-tuple-syntax
      base.Run();
//    ^^^^ no-unneeded-base-prefix
      var s = new S();
//            ^^^^^^^ no-default-value-type-constructor
      var d = new DateTime();
//            ^^^^^^^^^^^^^^ no-default-value-type-constructor
      var g = ValueTuple.Create(1, 2);
//            ^^^^^^^^^^^^^^^^^ use-tuple-syntax
      Debug.Assert(true);
//    ^^^^^^^^^^^^^^^^^^ debug-assert-message
      Debug.Assert(true, "why");
      Debug.Fail("");
//    ^^^^^^^^^^^^^^ debug-fail-message
    }
    /// <inheritdoc/>
//      ^^^^^^^^^^^^^ inheritdoc-on-inheriting-member
    public void Own() { }
  }
}
namespace C.D { }
namespace A.C.D { }
