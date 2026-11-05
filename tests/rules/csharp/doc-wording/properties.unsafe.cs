using System;
/// <summary>Properties.</summary>
public class Properties
{
    /// <summary>Gets or sets the count.</summary>
    public int Both { get; set; }
    /// <summary>Gets or sets the count.</summary>
    public int GoodBoth { get; set; }
    /// <summary>gets or sets the count.</summary>
    public int LowerBoth { get; set; }
    /// <summary>Gets the count.</summary>
    public int GetterOnly { get; }
    /// <summary>Gets the count.</summary>
    public int NoPrefixGetter { get; }
    /// <summary>Gets the count.</summary>
    public int PrivateSet { get; private set; }
    /// <summary>Gets the count.</summary>
    public int GoodPrivateSet { get; private set; }
    /// <summary>Gets the count.</summary>
    public int WrongPrivateSet { get; private set; }
    /// <summary>Sets the count.</summary>
    public int PrivateGet { private get; set; }
    /// <summary>Sets the count.</summary>
    public int GoodPrivateGet { private get; set; }
    /// <summary>Sets the count.</summary>
    public int WrongPrivateGet { private get; set; }
    /// <summary>Gets the count.</summary>
    public int GetterOnlyWrong { get; }
    /// <summary>Sets the count.</summary>
    public int SetterOnly { set { } }
    /// <summary>Sets the count.</summary>
    public int SetterOnlyWrong { set { } }
    /// <summary>Sets the count.</summary>
    public int SetterOnlyBoth { set { } }
    /// <summary>Gets the count.</summary>
    public int ReturnsPrefix { get; }
    /// <summary>Gets or sets the count.</summary>
    public int ReturnsBoth { get; set; }
    /// <summary>Gets a value indicating whether it is ready.</summary>
    public bool Ready { get; }
    /// <summary>Gets or sets a value indicating whether it is ready.</summary>
    public bool ReadyBoth { get; set; }
    /// <summary>Gets a value indicating whether gets the ready flag.</summary>
    public bool ReadyPlain { get; }
    /// <summary>Gets or sets a value indicating whether gets or sets the ready flag.</summary>
    public bool ReadyPlainBoth { get; set; }
    /// <summary>Gets or sets a value indicating whether it is ready.</summary>
    public bool ReadyWrongBoth { get; set; }
    /// <summary>Gets a value indicating whether it is ready.</summary>
    public bool ReadyPrivateSet { get; private set; }
    /// <summary>Gets a value indicating whether it is ready.</summary>
    public System.Boolean QualifiedBool { get; }
    /// <summary>Gets a value indicating whether it is ready.</summary>
    public Boolean NamedBool { get; }
}
