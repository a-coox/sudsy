namespace DocSummary
{
    /// <remarks>No summary.</remarks>
    public class NoSummary
    {
        /// <remarks>No summary.</remarks>
        public int Field;

        /// <remarks>No summary.</remarks>
        public int First, Second;

        /// <remarks>No summary.</remarks>
        public NoSummary() { }

        /// <remarks>No summary.</remarks>
        ~NoSummary() { }

        /// <remarks>No summary.</remarks>
        public int this[int index] => index;

        /// <remarks>No summary.</remarks>
        public event System.EventHandler Changed;

        /// <remarks>No summary.</remarks>
        public delegate void Callback();

        /// <remarks>No summary.</remarks>
        public enum Kind
        {
            /// <remarks>No summary.</remarks>
            A,
        }

        /// <remarks>No summary.</remarks>
        private int hidden;

        /// <remarks>No summary.</remarks>
        internal int Internal;

        /// <remarks>No summary.</remarks>
        protected int Protected;

        /// <remarks>No summary.</remarks>
        private class PrivateType
        {
            /// <remarks>No summary.</remarks>
            public int Field;
        }

        /// <remarks>No summary.</remarks>
        public interface IInterface
        {
            /// <remarks>No summary.</remarks>
            void Run();
        }
    }

    /// <remarks>No summary, internal.</remarks>
    internal class InternalNoSummary { }

    /// <remarks>No summary, private members.</remarks>
    internal class InternalHolder
    {
        /// <remarks>No summary.</remarks>
        public int Field;
    }

    /// <inheritdoc />
    public class Inherited : NoSummary { }

    /// <summary></summary>
    public class EmptySummary
    {
        /// <summary>
        /// </summary>
        public void Blank() { }

        /// <summary />
        public void SelfClosing() { }

        /// <summary><see cref="Blank"/></summary>
        public void OnlyCref() { }

        /// <summary>
        /// <!-- comment -->
        /// </summary>
        public void OnlyComment() { }

        /// <summary>   </summary>
        public int Field;

        /// <summary></summary>
        public int First, Second;
    }

    /// <summary>Summary description for Defaults.</summary>
    public class Defaults
    {
        /// <summary>
        /// Summary description for Method.
        /// </summary>
        public void Method() { }

        /// <summary><see cref="Method"/> Summary description for x</summary>
        public void NotFirst() { }

        /// <summary>summary description for lower case</summary>
        public void LowerCase() { }

        /// <summary>
        ///   Summary description for indented.
        /// </summary>
        public int Property { get; }

        /// <summary>Summary description for.</summary>
        /// <inheritdoc />
        public override string ToString() => "";
    }

    /// <remarks>Partial without summary.</remarks>
    public partial class PartialNoSummary { }

    /// <summary></summary>
    public partial class PartialEmptySummary { }

    /// <content>Content.</content>
    public partial class PartialContent { }

    /// <content></content>
    public partial class PartialEmptyContent { }

    /// <inheritdoc />
    public partial class PartialInherited { }

    /// <summary>Summary description for PartialDefault.</summary>
    public partial class PartialDefault { }

    /// <remarks>Internal partial.</remarks>
    internal partial struct PartialInternal { }

    public partial class PartialUndocumented { }

    internal partial struct PartialUndocumentedInternal { }

    /// <summary>Part one.</summary>
    public partial class PartialMembers
    {
        public partial void Declared();

        partial void Private();

        /// <summary>Documented.</summary>
        public partial void Implemented();

        /// <remarks>Without summary.</remarks>
        public partial void NoSummary();

        /// <summary></summary>
        public partial void EmptySummary();
    }

    /// <summary>Part two.</summary>
    public partial class PartialMembers
    {
        public partial void Declared() { }

        partial void Private() { }

        /// <summary>Documented.</summary>
        public partial void Implemented() { }

        /// <remarks>Without summary.</remarks>
        public partial void NoSummary() { }

        /// <summary></summary>
        public partial void EmptySummary() { }
    }
}
