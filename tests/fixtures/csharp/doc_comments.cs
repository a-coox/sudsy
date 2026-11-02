using System;

/// <summary>
/// A documented type with <c>inline</c> code, a <see cref="Exception"/> reference and
/// a <paramref name="x"/> that spans
/// several lines.
/// </summary>
/// <remarks>
/// <![CDATA[ raw <xml> & text ]]>
/// Entities: &lt; &amp; &#169;
/// </remarks>
/// <typeparam name="T">The type.</typeparam>
public class Documented<T>
{
    /** <summary>Block doc comment.</summary> */
    public void Block() { }

    /// <summary>A method.</summary>
    /// <param name="x">The input.</param>
    /// <returns>Nothing</returns>
    /// <exception cref="ArgumentException">Thrown when <c>x</c> is bad.</exception>
    public int Method(int x) => x;

    /// <summary>
    /// <list type="bullet">
    ///   <item><description>one</description></item>
    ///   <item><description>two</description></item>
    /// </list>
    /// </summary>
    /// <inheritdoc cref="Method(int)"/>
    public int Other => 0;

    // Not a doc comment.
    //// Neither is this.
    /// <summary>Trailing doc</summary>
    int field; /// not documentation after code
}
