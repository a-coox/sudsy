using System;
/// <summary>Text.</summary>
public class Duplicates
{
    /// <summary>The value.</summary>
    /// <param name="a">The value.</param>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-duplicate-doc-text
    /// <param name="b">The value.</param>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-duplicate-doc-text
    /// <param name="c">The parameter is not used.</param>
    /// <param name="d">The parameter is not used.</param>
    /// <param name="e" />
    /// <param name="f" />
    /// <returns>Other.</returns>
    public int Dup(int a, int b, int c, int d, int e, int f) => 0;
    /// <summary>The  value.</summary>
    /// <remarks>The
//      ^ no-duplicate-doc-text
    /// value.</remarks>
    public void Spacing() { }
    /// <summary>Inline <see cref="Duplicates"/> text.</summary>
    /// <remarks>Inline <see cref="Duplicates"/> text.</remarks>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-duplicate-doc-text
    /// <seealso cref="Duplicates"/>
    /// <seealso cref="Duplicates"/>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-duplicate-doc-text
    public void Inline() { }
    /// <summary>Same.</summary>
    /// <summary>Same.</summary>
//      ^^^^^^^^^^^^^^^^^^^^^^^^ no-duplicate-doc-text
    public void TwoSummaries() { }
    /// <summary>Same.</summary>
    /// <inheritdoc />
    /// <inheritdoc />
//      ^^^^^^^^^^^^^^ no-duplicate-doc-text
    public void Inherited() { }
}
