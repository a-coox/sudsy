using System;

namespace Fx.OrderMembersAccessors
{
    public class OrderMembersAccessors
    {
        public int A { set { } get { return 1; } }

        public int B { get; set; }

        public int C { init { } get { return 1; } }

        public int this[int index] { set { } get { return 1; } }

        public int this[string index] { get { return 1; } set { } }

        public event Action D { remove { } add { } }

        public event Action E { add { } remove { } }

        public int F { [Obsolete] set { } get { return 1; } }

        public int G { set; get; }

        public int H { get; }

        public int I { private set; get; }
    }
}
