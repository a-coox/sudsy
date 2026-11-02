using System;
using Alias = System.Collections.Generic.Dictionary<string, int>;

namespace Decls
{
    public delegate TResult Fn<in T, out TResult>(T arg) where TResult : class;

    public interface IShape<T> : IEquatable<T>, IDisposable where T : struct
    {
        double Area { get; }
        void Draw(ref int x, out int y, in int z, params int[] rest);
        static abstract T Create();
    }

    public abstract class Shape : IShape<int>
    {
        public const int Max = 10, Min = 0;
        protected internal static readonly object Lock = new();
        private volatile int _v;
        public event EventHandler Changed { add { } remove { } }
        public int this[int i, string s] { get => i; set { } }
        public abstract double Area { get; }
        public static Shape operator +(Shape a, Shape b) => a;
        public static implicit operator int(Shape s) => 0;
        public static explicit operator Shape(int i) => null!;
        ~Shape() { }
        public Shape() : this(1) { }
        public Shape(int i) : base() { }
        static Shape() { }
        public virtual unsafe void Fixed(int* p, delegate*<int, void> fp) { }
        public async Task<int> RunAsync() { await Task.Yield(); return 1; }
        public int Local() { int Inner(int a) => a; static int S() => 1; return Inner(S()); }
        public required string Name { get; init; }
        public int Computed => 1;
        int Backing { get => _v; set => _v = value; }
        extern void Native();
        partial void Hook();
        public ref readonly int RefReturn(scoped ref int x) => ref x;
    }

    public struct S { public int X; public readonly int Y => X; }
    public enum E { A, B = 2, C }
}
