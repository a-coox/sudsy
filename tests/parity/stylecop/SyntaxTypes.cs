namespace Fx.SyntaxTypes
{
    public class SyntaxTypes { }

    public struct StructType { }

    public interface IInterfaceType { }

    public class SecondClass { }

    public record RecordClass(int X);

    public record struct RecordStruct(int X);

    public partial class Parts { }

    public partial class Parts { }

    public delegate void DelegateType();

    public class Generic<T> { }

    namespace Inner
    {
        public class InnerClass { }
    }
}

namespace Fx.SyntaxTypes.Second
{
    public enum EnumType { A }
}
