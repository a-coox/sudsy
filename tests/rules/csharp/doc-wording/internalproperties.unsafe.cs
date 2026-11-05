using System;
/// <summary>Properties of an internal class.</summary>
internal class InternalProperties
{
    /// <summary>Gets or sets the count.</summary>
    protected internal int Both { get; protected set; }

    /// <summary>Gets or sets the count.</summary>
    protected internal int GoodBoth { get; protected set; }

    /// <summary>Gets or sets the count.</summary>
    public int PublicBoth { get; internal set; }
}
