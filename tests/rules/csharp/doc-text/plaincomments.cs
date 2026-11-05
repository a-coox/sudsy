using System;
/// <summary>Text.</summary>
public class PlainComments
{
    /// plain text on a field
//  ^^^ no-doc-slashes-in-plain-comment
    public int Field;
    /// first line
//  ^^^ no-doc-slashes-in-plain-comment
    /// second line
//  ^^^ no-doc-slashes-in-plain-comment
    public void Attached() { }
    /// <summary>Good.</summary>
    /// extra text after the summary
    public void Mixed() { }
    void Body()
    {
        /// plain in a body
//      ^^^ no-doc-slashes-in-plain-comment
        int j = 0;

        ///
//      ^^^ no-doc-slashes-in-plain-comment
        int k = 0;

        /// <summary>Not plain.</summary>
        int l = 0;
    }
    /** plain block */
    public int Block;
    //// four slashes
    public int Four;
    int last; ///trailing
//            ^^^ no-doc-slashes-in-plain-comment
}
/// <summary>Text.</summary>
public class Empties
{
    /// <summary>Text.</summary>
    /// <remarks></remarks>
//      ^^^^^^^^^^^^^^^^^^^ doc-text-not-empty
    /// <example />
//      ^^^^^^^^^^^ doc-text-not-empty
    /// <exception cref="System.Exception"></exception>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-text-not-empty
    /// <exception cref="System.ArgumentException">Thrown.</exception>
    /// <permission cref="System.Security.PermissionSet" />
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-text-not-empty
    /// <remarks>
//      ^ doc-text-not-empty
    /// </remarks>
    /// <remarks>Text.</remarks>
//      ^^^^^^^^^^^^^^^^^^^^^^^^ no-duplicate-doc-text
    /// <returns></returns>
    public int Elements() => 0;
    /// <summary>
    /// Text.
    /// <remarks></remarks>
//      ^^^^^^^^^^^^^^^^^^^ doc-text-not-empty
    /// </summary>
    public void Nested() { }
    /// <remarks></remarks>
//      ^^^^^^^^^^^^^^^^^^^ doc-text-not-empty
    int Hidden;
    /// <remarks><see cref="Hidden"/></remarks>
//                                   ^ doc-text-ends-with-period
    public int Cref;
    /// <remarks><!-- comment --></remarks>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-text-not-empty
    public int Comment;
}
