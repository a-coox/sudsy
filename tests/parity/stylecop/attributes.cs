using System;

[assembly: Parity.A, Parity.B]

namespace Parity
{
    [AttributeUsage(AttributeTargets.All, AllowMultiple = true)]
    public class AAttribute : Attribute { }

    [AttributeUsage(AttributeTargets.All)]
    public class BAttribute : Attribute { }

    [A, B]
    public class Combined { }

    [A]
    [B]
    public class Separate { }

    [A()]
    public class WithParens { }

    [A(), B()]
    public class Both { }

    public class Members
    {
        [A, B]
        public int P { get; set; }

        [return: A, B]
        public int M([A, B] int p) { return p; }

        [A(), B]
        public int Q;

        [Obsolete()]
        public int R;

        [Obsolete("x")]
        public int S;

        [A, B] // trailing
        public int T;

        [A, B, A]
        public int U;

        [A, // first
            B]
        public int V;

        public void Generic<[A, B] T>() { }

        public void Lambdas()
        {
            System.Action<int> a = ([A, B] int v) => { };
            [A, B] void Local() { }
        }

        public int Accessor { [A, B] get { return 1; } }

        [A,B]
        public int W;

        [A , B]
        public int X;
    }

    [A, B]
    public enum E { [A, B] One }

    [A, B]
    public delegate void D();

    public interface I
    {
        [A, B]
        void M();
        event System.Action Ev;
    }

    public struct S
    {
        [field: A, B]
        public int Y { get; set; }

        [Obsolete("x"), B]
        public int Z;

        [Obsolete()]
        [A()]
        public int Q3;
    }
}
