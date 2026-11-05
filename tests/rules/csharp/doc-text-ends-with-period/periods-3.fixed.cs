using System;
/// <summary>Text.</summary>
public class Periods
{
    /// <summary />
    public void SelfClosing() { }
    /// <summary>Text.</summary>
    /// <param name="a">Value.</param>
    /// <returns>Result.</returns>
    /// <remarks>Remarks.</remarks>
    /// <exception cref="Exception">Throws.</exception>
    /// <seealso cref="Periods">See also</seealso>
    /// <example>Example.</example>
    public int Elements(int a) => 0;
    /// <summary>Text.</summary>
    /// <example>
    /// <code>
    /// var x = 1
    /// </code>
    /// </example>
    public void Example() { }
    /// <summary>Text.</summary>
    /// <typeparam name="T">Type.</typeparam>
    public void Generic<T>() { }
    /// <inheritdoc />
    public override string ToString() => "";
    /// <summary>Inherited text.</summary>
    /// <inheritdoc />
    public override int GetHashCode() => 0;
    /// <summary>Field.</summary>
    public int Field;
    /// <summary>Property.</summary>
    public int Property { get; }
    /// <summary>Event</summary>
    public event EventHandler Changed;
    /// <summary>Destructor</summary>
    ~Periods() { }
    /// <summary>Enum.</summary>
    public enum Kind
    {
        /// <summary>Member.</summary>
        A,
    }
    /// <summary>Delegate.</summary>
    public delegate void Callback();
    /// <summary>Operator.</summary>
    public static Periods operator +(Periods a, Periods b) => a;
    /// <summary>Conversion.</summary>
    public static implicit operator int(Periods a) => 0;
    /// <summary>Indexer.</summary>
    public int this[int i] => i;
    /// <summary>Constructor.</summary>
    public Periods() { }
}
/// <summary>Struct.</summary>
public struct Struct { }
