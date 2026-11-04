using System;

namespace Fx.SyntaxFields
{
    public class Fields
    {
        public int E, F;
        private int g = 1, h = 2;
        public const int C1 = 1, C2 = 2;
        public static readonly string N1 = "a", N2 = "b";
        [Obsolete] public int J, K;
        public event EventHandler Ev1, Ev2;
        private int[] arr1, arr2;
        // comment
        int m, n; // trailing
        /// <summary>Doc</summary>
        public int O, P;
        int q, // c1
            r;
        public int single;

        public void Locals()
        {
            int a, b;
            int c = 1, d = 2;
            for (int i = 0, j = 1; i < 2; i++) { }
        }
    }
}
