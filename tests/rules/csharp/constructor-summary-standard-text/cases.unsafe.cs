using System;

namespace DocWording
{
    /// <summary>Properties.</summary>
    public class Properties
    {
        /// <summary>The count.</summary>
        public int Both { get; set; }

        /// <summary>Gets or sets the count.</summary>
        public int GoodBoth { get; set; }

        /// <summary>gets or sets the count.</summary>
        public int LowerBoth { get; set; }

        /// <summary>Gets the count.</summary>
        public int GetterOnly { get; }

        /// <summary>The count.</summary>
        public int NoPrefixGetter { get; }

        /// <summary>Gets or sets the count.</summary>
        public int PrivateSet { get; private set; }

        /// <summary>Gets the count.</summary>
        public int GoodPrivateSet { get; private set; }

        /// <summary>Sets the count.</summary>
        public int WrongPrivateSet { get; private set; }

        /// <summary>Gets or sets the count.</summary>
        public int PrivateGet { private get; set; }

        /// <summary>Sets the count.</summary>
        public int GoodPrivateGet { private get; set; }

        /// <summary>Gets the count.</summary>
        public int WrongPrivateGet { private get; set; }

        /// <summary>Gets or sets the count.</summary>
        public int GetterOnlyWrong { get; }

        /// <summary>Sets the count.</summary>
        public int SetterOnly { set { } }

        /// <summary>Gets the count.</summary>
        public int SetterOnlyWrong { set { } }

        /// <summary>Gets or sets the count.</summary>
        public int SetterOnlyBoth { set { } }

        /// <summary>Returns the count.</summary>
        public int ReturnsPrefix { get; }

        /// <summary>Returns the count.</summary>
        public int ReturnsBoth { get; set; }

        /// <summary>Gets a value indicating whether it is ready.</summary>
        public bool Ready { get; }

        /// <summary>Gets or sets a value indicating whether it is ready.</summary>
        public bool ReadyBoth { get; set; }

        /// <summary>Gets the ready flag.</summary>
        public bool ReadyPlain { get; }

        /// <summary>Gets or sets the ready flag.</summary>
        public bool ReadyPlainBoth { get; set; }

        /// <summary>Gets a value indicating whether it is ready.</summary>
        public bool ReadyWrongBoth { get; set; }

        /// <summary>Gets or sets a value indicating whether it is ready.</summary>
        public bool ReadyPrivateSet { get; private set; }

        /// <summary>Gets a value indicating whether it is ready.</summary>
        public System.Boolean QualifiedBool { get; }

        /// <summary>Gets a value indicating whether it is ready.</summary>
        public Boolean NamedBool { get; }

        /// <summary>Gets a value indicating whether it is ready.</summary>
        public bool? NullableBool { get; }

        /// <summary>Gets or sets the count.</summary>
        internal int InternalBoth { get; private set; }

        /// <summary>Gets or sets the count.</summary>
        protected int ProtectedPrivateSet { get; private set; }

        /// <summary>Gets or sets the count.</summary>
        protected internal int ProtectedInternalSet { get; internal set; }

        /// <summary>Gets or sets the count.</summary>
        public int InternalSet { get; internal set; }

        /// <summary>Gets or sets the count.</summary>
        public int ProtectedSet { get; protected set; }

        /// <summary>Gets the count.</summary>
        public int InternalGetPublicSet { internal get; set; }

        /// <summary>The expression.</summary>
        public int Expression => 1;

        /// <summary>Gets the expression.</summary>
        public int GoodExpression => 1;

        /// <summary>
        /// <see cref="Both"/> Gets the count.
        /// </summary>
        public int StartsWithSee { get; }

        /// <summary>
        /// Gets or sets the thing.
        /// </summary>
        public string MultiLine { get; set; }

        /// <summary>
        /// The thing.
        /// </summary>
        public string MultiLineWrong { get; set; }

        /// <summary></summary>
        public int Blank { get; }

        /// <summary />
        public int SelfClosing { get; }

        /// <inheritdoc />
        public int Inherited { get; }

        /// <summary>The count.</summary>
        /// <inheritdoc />
        public int InheritedWithSummary { get; }

        /// <summary>The count.</summary>
        private int PrivateProperty { get; set; }

        /// <summary>The count.</summary>
        int ImplicitPrivate { get; set; }

        /// <summary>Gets or sets the count.</summary>
        public int this[int index] { get => index; }

        /// <summary>The count.</summary>
        public ref int RefProperty => throw null;
    }

    /// <summary>Properties of an internal class.</summary>
    internal class InternalProperties
    {
        /// <summary>The count.</summary>
        protected internal int Both { get; protected set; }

        /// <summary>Gets or sets the count.</summary>
        protected internal int GoodBoth { get; protected set; }

        /// <summary>Gets or sets the count.</summary>
        public int PublicBoth { get; internal set; }
    }

    /// <summary>Constructors.</summary>
    public class Constructors<T>
    {
        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class.</summary>
        public Constructors() { }

        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class. Creates it.</summary>
        public Constructors(int a) { }

        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class.</summary>
        public Constructors(int a, int b) { }

        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class.</summary>
        public Constructors(int a, int b, int c) { }

        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class. Initializes a new instance of the Constructors class.</summary>
        public Constructors(long a) { }

        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class and more.</summary>
        public Constructors(long a, long b) { }

        /// <summary>
        /// Initializes a new instance of the <see cref="Constructors{T}"/> class.
        /// </summary>
        public Constructors(string a) { }

        /// <summary>
        /// Initializes a new instance of the
        /// <see cref="Constructors{T}"/> class.
        /// </summary>
        public Constructors(string a, string b) { }

        /// <summary>
        /// Initializes  a new instance of the <see cref="Constructors{T}"/>   class.
        /// </summary>
        public Constructors(string a, string b, string c) { }

        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class.</summary>
        /// <remarks>More.</remarks>
        internal Constructors(char a) { }

        /// <summary>Initializes static members of the <see cref="Constructors{T}"/> class.</summary>
        static Constructors() { }

        /// <summary>Prevents a default instance of the <see cref="Constructors{T}"/> class from being created.</summary>
        private Constructors(char a, char b) { }

        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class.</summary>
        private Constructors(char a, char b, char c) { }

        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class. Private.</summary>
        private Constructors(char a, char b, char c, char d) { }

        /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class. Prevents a default instance of the <see cref="Constructors{T}"/> class from being created.</summary>
        protected Constructors(byte a) { }

        /// <summary>Finalizes an instance of the <see cref="Constructors{T}"/> class.</summary>
        ~Constructors() { }

        /// <inheritdoc />
        protected Constructors(byte a, byte b) { }

        /// <summary />
        protected Constructors(byte a, byte b, byte c) { }

        /// <remarks>No summary.</remarks>
        protected Constructors(byte a, byte b, byte c, byte d) { }
    }

    /// <summary>Static.</summary>
    public class StaticWrong
    {
        /// <summary>Initializes static members of the <see cref="StaticWrong"/> class. Static.</summary>
        static StaticWrong() { }
    }

    /// <summary>Struct.</summary>
    public struct Pair
    {
        /// <summary>Initializes a new instance of the <see cref="Pair"/> struct.</summary>
        public Pair(int a) { }

        /// <summary>Initializes a new instance of the <see cref="Pair"/> struct.</summary>
        public Pair(int a, int b) { }

        /// <summary>Prevents a default instance of the <see cref="Pair"/> struct from being created.</summary>
        private Pair(int a, int b, int c) { }

        /// <summary>Initializes static members of the <see cref="Pair"/> struct.</summary>
        static Pair() { }
    }

    /// <summary>Destructors.</summary>
    public class Destructors<T>
    {
        /// <summary>Finalizes an instance of the <see cref="Destructors{T}"/> class.</summary>
        ~Destructors() { }
    }

    /// <summary>Destructors.</summary>
    public class DestructorsWrong
    {
        /// <summary>Cleans up.</summary>
        ~DestructorsWrong() { }
    }

    /// <summary>Destructors.</summary>
    public class DestructorsOther
    {
        /// <summary>Finalizes an instance of the <see cref="Other"/> class.</summary>
        ~DestructorsOther() { }
    }

    /// <summary>Destructors.</summary>
    public class DestructorsMultiLine
    {
        /// <summary>
        /// Finalizes an instance of the <see cref="DestructorsMultiLine"/> class.
        /// </summary>
        ~DestructorsMultiLine() { }
    }

    /// <summary>Destructors.</summary>
    public class DestructorsNoPeriod
    {
        /// <summary>Finalizes an instance of the <see cref="DestructorsNoPeriod"/> struct.</summary>
        ~DestructorsNoPeriod() { }
    }

    /// <summary>Other.</summary>
    public class Other { }
}
