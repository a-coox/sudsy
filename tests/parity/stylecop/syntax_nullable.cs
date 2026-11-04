using System;
using System.Collections.Generic;
using Alias = System.Nullable<int>;

namespace Fx.SyntaxNullable
{
    public struct Value { }

    public class Nullables
    {
        public Nullable<int> A;
        public System.Nullable<int> B;
        public global::System.Nullable<int> C;
        public List<Nullable<int>> D;
        public Nullable<int>[] E;
        public Type F = typeof(Nullable<>);
        public Type G = typeof(Nullable<int>);
        public string H = nameof(Nullable<int>);
        public Nullable<int> Make() { return new Nullable<int>(1); }
        public bool Same(Nullable<int> a) { return Nullable<int>.Equals(a, a); }
        public Nullable<int> Prop { get; set; }
        public static implicit operator Nullable<long>(Nullables n) { return null; }
        public Nullable<Value> Custom;
        public int? Short;
        public Type Closed = typeof(int?);

        public void Locals(object o)
        {
            Nullable<int> local = null;
            var x = default(Nullable<int>);
            if (o is Nullable<int>) { }
            var y = (Nullable<int>)o;
            Alias z = null;
        }
    }
}
