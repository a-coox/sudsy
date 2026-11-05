using System;
/// <summary>Text.</summary>
public class Placeholders
{
    /// <summary>
    /// <placeholder>todo</placeholder>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-doc-placeholder
    /// <placeholder />
//      ^^^^^^^^^^^^^^^ no-doc-placeholder
    /// </summary>
    public void Placeholder() { }
    /// <placeholder>todo</placeholder>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-doc-placeholder
//                       ^ doc-text-ends-with-period
    private int Hidden;
    /// <summary>
    /// <para><placeholder>nested</placeholder></para>
//            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-doc-placeholder
    /// </summary>
    public int Nested;
}
