using System;

namespace Fx.OrderMembersKind
{
    public class OrderMembersKind
    {
        public void Method() { }

        public int field;

        public OrderMembersKind() { }

        public int Property { get; set; }

        ~OrderMembersKind() { }

        public delegate void Handler();

        public event Action Event;

        public int this[int index] => index;

        public static OrderMembersKind operator +(OrderMembersKind a, OrderMembersKind b) => a;

        public static implicit operator int(OrderMembersKind a) => 0;

        public enum Kind { A }

        public interface IInner { }

        public struct Inner { }

        public class InnerClass { }

        public Action EventField2;

        public event Action EventField3, EventField4;

        public record InnerRecord(int X);

        public record struct InnerRecordStruct(int X);

        public int AfterTypes;
    }

    public struct OrderMembersKindStruct
    {
        public int Property { get; set; }

        private int field;
    }

    public interface IOrderMembersKind
    {
        void Method();

        int Property { get; }

        event Action Event;
    }

    public record OrderMembersKindRecord(int X)
    {
        public void Method() { }

        public int Field;
    }
}

namespace Fx.OrderMembersKindOuter
{
    public class OuterClass { }

    public struct OuterStruct { }

    public interface IOuterInterface { }

    public enum OuterEnum { A }

    public delegate void OuterDelegate();

    namespace Nested { }

    public record OuterRecord(int X);

    public delegate void SecondDelegate();
}
