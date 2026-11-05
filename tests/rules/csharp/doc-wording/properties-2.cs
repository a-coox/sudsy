using System;
/// <summary>Properties.</summary>
public class Properties
{
    /// <summary>Gets a value indicating whether it is ready.</summary>
    public bool? NullableBool { get; }
    /// <summary>Gets or sets the count.</summary>
    internal int InternalBoth { get; private set; }
//               ^^^^^^^^^^^^ property-summary-omits-restricted-accessor
    /// <summary>Gets or sets the count.</summary>
    protected int ProtectedPrivateSet { get; private set; }
//                ^^^^^^^^^^^^^^^^^^^ property-summary-omits-restricted-accessor
    /// <summary>Gets or sets the count.</summary>
    protected internal int ProtectedInternalSet { get; internal set; }
//                         ^^^^^^^^^^^^^^^^^^^^ property-summary-omits-restricted-accessor
    /// <summary>Gets or sets the count.</summary>
    public int InternalSet { get; internal set; }
//             ^^^^^^^^^^^ property-summary-omits-restricted-accessor
    /// <summary>Gets or sets the count.</summary>
    public int ProtectedSet { get; protected set; }
    /// <summary>Gets the count.</summary>
    public int InternalGetPublicSet { internal get; set; }
//             ^^^^^^^^^^^^^^^^^^^^ property-summary-matches-accessors
    /// <summary>The expression.</summary>
    public int Expression => 1;
//             ^^^^^^^^^^ property-summary-matches-accessors
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
//                ^^^^^^^^^^^^^^ property-summary-matches-accessors
    /// <summary></summary>
    public int Blank { get; }
//             ^^^^^ property-summary-matches-accessors
    /// <summary />
    public int SelfClosing { get; }
    /// <inheritdoc />
    public int Inherited { get; }
    /// <summary>The count.</summary>
    /// <inheritdoc />
    public int InheritedWithSummary { get; }
    /// <summary>The count.</summary>
    private int PrivateProperty { get; set; }
//              ^^^^^^^^^^^^^^^ property-summary-matches-accessors
    /// <summary>The count.</summary>
    int ImplicitPrivate { get; set; }
//      ^^^^^^^^^^^^^^^ property-summary-matches-accessors
    /// <summary>Gets or sets the count.</summary>
    public int this[int index] { get => index; }
    /// <summary>The count.</summary>
    public ref int RefProperty => throw null;
//                 ^^^^^^^^^^^ property-summary-matches-accessors
}
