using System;
using System.Diagnostics;
namespace A.B {
  using C.D;
  struct S { }
  interface I { (int, int) Pair(); }
  class P { public virtual void Run() { } }
  class Q : P, I {
    public (int, int) Pair() => (1, 2);
    public (int, int) Other() => (1, 2);
    public void Go(ValueTuple<int, int> t) {
//                 ^^^^^^^^^^^^^^^^^^^^ use-tuple-syntax
      base.Run();
      var s = new S();
      var d = new DateTime();
      var g = ValueTuple.Create(1, 2);
//            ^^^^^^^^^^^^^^^^^ use-tuple-syntax
      Debug.Assert(true);
      Debug.Assert(true, "why");
      Debug.Fail("");
    }
    /// <inheritdoc/>
    public void Own() { }
  }
}
namespace C.D { }
namespace A.C.D { }
