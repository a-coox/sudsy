using System;
using System.Collections.Generic;

// Every construct that makes the compiler report CS0162, CS0168 or CS0219, and lookalikes that it
// does not report. compiler_agreement.rs checks that the flow rules
// report the same places.
class Z
{
    const bool C = false;

    int A1() { return 1; int dead = 2; }
    void A2() { throw new Exception(); A2(); }
    void A3() { while (true) { } A3(); }
    void A4(int x) { for (;;) { if (x > 0) break; } A4(1); }
    void A5() { if (C) { A5(); } }
    void A6(int x) { switch (x) { case 1: return; default: return; } A6(1); }
    void A7() { try { A7(); } finally { A7(); } A7(); }
    void A8() { try { throw new Exception(); } catch { } A8(); }
    int A9(bool b) { do { return 1; } while (b); }
    void A10() { goto end; A10(); end: A10(); }
    void A11() { int u1; int u2 = 1; int u3 = 2; u3 = 3; int u4 = 0; u4++; Console.WriteLine(u4); }
    void A12() { try { } catch (Exception e) { } catch { } }
    void A13() { Action f = () => { return; A13(); }; f(); }
    void A14() { int c = 1; Action f = () => Console.WriteLine(c); f(); }
    void A15() { foreach (var q in new int[0]) { } for (int i = 0; ; ) { break; } }
    IEnumerable<int> A16() { yield return 1; yield break; A16(); }
    void A17(object o) { if (o is string s) { } if (o is int n && n > 0) { } }
    void A18() { using var d = new System.IO.MemoryStream(); }
    void A19() { string s = null; int? v = null; object o = new object(); }
    void A20(int x) { while (x > 0) { x--; continue; x++; } }
    void A21() { lock (this) { return; A21(); } }
    void A22() { L: goto L; }
    void A23(int x) { if (x > 0) { return; } else { return; } A23(1); }
    void A24() { var (a, b) = (1, 2); Console.WriteLine(a); }
    void A25() { int x = 1; { int y = 2; } int z; z = 3; Console.WriteLine(x); }
    void A26(bool b) { while (false) { A26(b); } }
    void A27() { for (int i = 0; false; i++) { A27(); } }
    void A28(int x) { switch (x) { case 1: A28(1); break; A28(2); } }
    void A29() { int k = 5; k = 6; }
    void A30() { void Local() { int inner = 1; } Local(); }
    int A31(int x) => x > 0 ? 1 : throw new Exception();
    void A32() { try { return; } finally { A32(); } }
    void A33() { Exception e = null; try { } catch (Exception ex) { Console.WriteLine(ex); } }
    void A34() { int x; if (C) { x = 1; } }
    void A35() { throw null; ; }
    void A37() { return; throw new Exception(); }
    void A38() { while (true) { } throw new Exception(); }
    void A36() { out_(out int r); }
    void out_(out int r) { r = 1; }
}
