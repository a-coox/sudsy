using System;
/// <summary>Properties.</summary>
public class Properties
{
    /// <summary>Gets a value indicating whether it is ready.</summary>
    public bool? NullableBool { get; }
    /// <summary>Gets the count.</summary>
    internal int InternalBoth { get; private set; }
    /// <summary>Gets the count.</summary>
    protected int ProtectedPrivateSet { get; private set; }
    /// <summary>Gets the count.</summary>
    protected internal int ProtectedInternalSet { get; internal set; }
    /// <summary>Gets the count.</summary>
    public int InternalSet { get; internal set; }
    /// <summary>Gets or sets the count.</summary>
    public int ProtectedSet { get; protected set; }
    /// <summary>Sets the count.</summary>
    public int InternalGetPublicSet { internal get; set; }
    /// <summary>Gets the expression.</summary>
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
    /// Gets or sets the thing.
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
    /// <summary>Gets or sets the count.</summary>
    private int PrivateProperty { get; set; }
    /// <summary>Gets or sets the count.</summary>
    int ImplicitPrivate { get; set; }
    /// <summary>Gets or sets the count.</summary>
    public int this[int index] { get => index; }
    /// <summary>Gets the count.</summary>
    public ref int RefProperty => throw null;
}
