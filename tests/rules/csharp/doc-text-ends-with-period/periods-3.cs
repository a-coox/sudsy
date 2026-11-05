using System;
/// <summary>Text.</summary>
public class Periods
{
    /// <summary />
    public void SelfClosing() { }
    /// <summary>Text.</summary>
    /// <param name="a">Value</param>
//                           ^ doc-text-ends-with-period
    /// <returns>Result</returns>
//                     ^ doc-text-ends-with-period
    /// <remarks>Remarks</remarks>
//                      ^ doc-text-ends-with-period
    /// <exception cref="Exception">Throws</exception>
//                                        ^ doc-text-ends-with-period
    /// <seealso cref="Periods">See also</seealso>
    /// <example>Example</example>
//                      ^ doc-text-ends-with-period
    public int Elements(int a) => 0;
    /// <summary>Text.</summary>
    /// <example>
    /// <code>
    /// var x = 1
    /// </code>
    /// </example>
    public void Example() { }
    /// <summary>Text.</summary>
    /// <typeparam name="T">Type</typeparam>
//                              ^ doc-text-ends-with-period
    public void Generic<T>() { }
    /// <inheritdoc />
    public override string ToString() => "";
    /// <summary>Inherited text</summary>
//                             ^ doc-text-ends-with-period
    /// <inheritdoc />
    public override int GetHashCode() => 0;
    /// <summary>Field</summary>
//                    ^ doc-text-ends-with-period
    public int Field;
    /// <summary>Property</summary>
//                       ^ doc-text-ends-with-period
    public int Property { get; }
    /// <summary>Event</summary>
    public event EventHandler Changed;
    /// <summary>Destructor</summary>
    ~Periods() { }
    /// <summary>Enum</summary>
//                   ^ doc-text-ends-with-period
    public enum Kind
    {
        /// <summary>Member</summary>
//                         ^ doc-text-ends-with-period
        A,
    }
    /// <summary>Delegate</summary>
//                       ^ doc-text-ends-with-period
    public delegate void Callback();
    /// <summary>Operator</summary>
//                       ^ doc-text-ends-with-period
    public static Periods operator +(Periods a, Periods b) => a;
    /// <summary>Conversion</summary>
//                         ^ doc-text-ends-with-period
    public static implicit operator int(Periods a) => 0;
    /// <summary>Indexer</summary>
//                      ^ doc-text-ends-with-period
    public int this[int i] => i;
    /// <summary>Constructor</summary>
//                          ^ doc-text-ends-with-period
    public Periods() { }
}
/// <summary>Struct</summary>
//                 ^ doc-text-ends-with-period
public struct Struct { }
