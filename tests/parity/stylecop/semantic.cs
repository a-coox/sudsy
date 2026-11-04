// Rules that need the type layer: SA1100, SA1129, SA1135, SA1141, SA1405, SA1406, SA1414, SA1648.
namespace Outer
{
    using System;
    using System.Diagnostics;
    using System.Threading;

    public interface IPair
    {
        (int, int) Both();
    }

    public class Animal
    {
        public virtual string Name() => "animal";

        public string Kind() => "kind";

        public int this[int i] => i;

        public virtual int Count { get; }
    }

    public enum Color
    {
        Red = 0,
        Green = 1,
    }

    public enum Pair
    {
        A = 0,
        B = 0,
    }

    public struct Point
    {
        public int X;
    }

    public struct Explicit
    {
        public Explicit()
        {
        }
    }

    /// <summary>A dog.</summary>
    public class Dog : Animal, IPair
    {
        public override string Name() => base.Name();

        public string Sound() => base.Kind();

        public int Item() => base[1];

        public override int Count => base.Count;

        public (int, int) Both() => (1, 2);

        public (int a, int b) Named() => (1, 2);

        public (int, string) Unnamed() => (1, "x");

        public void Take((int, int) pair, (int x, int y) named)
        {
        }

        public (int, int) Property { get; }

        public System.ValueTuple<int, int> Old() => default;

        public ValueTuple<int, ValueTuple<int, int>> Nested() => default;

        public ValueTuple<int> Single() => default;

        public void Make()
        {
            ValueTuple<int, int> a = default;
            var b = ValueTuple.Create(1, 2);
            var c = ValueTuple.Create(1);
            var d = (ValueTuple<int, int>)(1, 2);
            var e = new ValueTuple<int, int>(1, 2);
            var f = default(ValueTuple<int, int>);
        }

        public void Defaults()
        {
            var a = new DateTime();
            var b = new Point();
            var c = new Point { X = 1 };
            var d = new Explicit();
            var e = new Guid();
            var f = new CancellationToken();
            var g = new Color();
            var h = new Pair();
            var i = new IntPtr();
            var j = new object();
            var k = new DateTime(1);
            Point l = new();
        }

        public void Generic<T>()
            where T : struct
        {
            var a = new T();
        }

        public void Generic2<T>()
            where T : new()
        {
            var a = new T();
        }

        public void Debugging(bool ok)
        {
            Debug.Assert(ok);
            Debug.Assert(ok, "message");
            Debug.Assert(ok, string.Empty);
            Debug.Assert(ok, " ");
            Debug.Assert(ok, "message", "detail");
            Debug.Fail("message");
            Debug.Fail(string.Empty);
            Debug.Fail("   ", "detail");
        }

        public void Defaults(Point p = new Point())
        {
        }

        public Dog()
        {
        }

        public Dog(int x)
            : base()
        {
        }
    }

    public class Docs : Animal
    {
        /// <inheritdoc/>
        public Docs()
        {
        }

        /// <inheritdoc/>
        public override string Name() => "docs";

        /// <inheritdoc/>
        public string Own() => "own";

        /// <inheritdoc cref="Animal.Kind"/>
        public string Other() => "other";

        /// <inheritdoc/>
        public int Field;

        /// <inheritdoc/>
        public event EventHandler Raised;

        /// <inheritdoc/>
        public int Prop { get; }
    }

    /// <inheritdoc/>
    public class Plain
    {
    }

    /// <inheritdoc/>
    public class Derived : Animal
    {
    }

    public class Impl : IPair
    {
        /// <inheritdoc/>
        public (int, int) Both() => (1, 2);

        /// <inheritdoc/>
        (int, int) IPair.Both() => (3, 4);
    }
}

namespace Outer.Inner
{
    using System.Collections.Generic;
    using Deep;
    using static System.Math;
    using Alias = System.Text.StringBuilder;
    using Dict = System.Collections.Generic.Dictionary<string, int>;

    public class User
    {
    }
}

namespace Outer.Inner.Deep
{
    public class Marker
    {
        public static int Value;
    }
}

namespace Third
{
    using System.Text;
    using Outer.Inner;
    using static Outer.Inner.Deep.Marker;
}
