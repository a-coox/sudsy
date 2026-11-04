using System;

namespace Fx.OrderMembersConstStatic
{
    public class OrderMembersConstStatic
    {
        public int a;

        public const int B = 1;

        public static int c;

        public readonly int d = 0;

        public static readonly int E = 0;

        public const int F = 2;

        private int g;

        private const int H = 3;

        private static int i;

        private readonly int j = 0;

        private int k;

        private readonly int l = 0;

        public static void Static1() { }

        public void Instance1() { }

        public static void Static2() { }

        public static int StaticProperty { get; }

        public int InstanceProperty { get; }

        public static int StaticProperty2 { get; }

        private static readonly int m = 0;

        private static int n;

        private static readonly int o = 0;

        protected static int p;

        protected int q;

        protected static int r;
    }
}
