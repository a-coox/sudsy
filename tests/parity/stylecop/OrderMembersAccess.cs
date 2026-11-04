using System;

namespace Fx.OrderMembersAccess
{
    public class OrderMembersAccess : IComparable
    {
        private int a;

        public int b;

        internal int c;

        protected int d;

        protected internal int e;

        private protected int f;

        int g;

        public int h;

        private void M1() { }

        public void M2() { }

        public static OrderMembersAccess operator -(OrderMembersAccess x, OrderMembersAccess y) => x;

        static OrderMembersAccess() { }

        internal OrderMembersAccess(int x) { }

        public OrderMembersAccess() { }

        private class PrivateClass { }

        public class PublicClass { }

        private int Pa { get; }

        public int Pb { get; }

        int IComparable.CompareTo(object other) => 0;
    }

    internal class Internal { }

    public class PublicAfter { }

    class Implicit { }

    public class PublicAfterImplicit { }
}
