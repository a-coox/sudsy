namespace Fx.SyntaxLiterals
{
    public class Literals
    {
        public void Casts()
        {
            var c1 = (long)1;
            var c2 = (ulong)1;
            var c3 = (uint)1;
            var c4 = (float)1;
            var c5 = (double)1;
            var c6 = (decimal)1;
            var c7 = (int)1;
            var c8 = (byte)1;
            var c10 = (long)-1;
            var c11 = (float)1.5;
            var c12 = (double)1.5;
            var c13 = (decimal)1.5;
            var c14 = (long)0x10;
            var c15 = (float)0x10;
            var c16 = (double)0x10;
            var c17 = (long)1L;
            var c18 = (float)1f;
            var c19 = (double)1d;
            var c20 = (uint)1u;
            var c21 = (ulong)1UL;
            var c22 = (System.Int64)1;
            var c25 = (ushort)1;
            var c27 = (long)(1);
            var c28 = (long)+1;
            var c29 = (double)1e3;
            var c30 = (float)1e3;
            var c31 = (decimal)1e3;
            var c33 = (long)1_000;
            var c34 = (float)1_0.5;
            var c35 = (double)0b101;
            var c36 = (decimal)0x1f;
            var c37 = (float)'c';
            var c38 = (long)"a".Length;
            var c39 = (long)-1 + 2;
            var c40 = -(long)1;
            var c42 = (double).5;
            var c43 = (decimal)1m;
            var c44 = (float)1D;
            var c45 = (long)1U;
            var c46 = (long)1 /* c */;
        }
    }
}
