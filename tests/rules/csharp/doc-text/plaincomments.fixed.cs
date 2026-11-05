using System;
/// <summary>Text.</summary>
public class PlainComments
{
    /// plain text on a field
    public int Field;
    /// first line
    /// second line
    public void Attached() { }
    /// <summary>Good.</summary>
    /// extra text after the summary
    public void Mixed() { }
    void Body()
    {
        // plain in a body
        int j = 0;

        //
        int k = 0;

        /// <summary>Not plain.</summary>
        int l = 0;
    }
    /** plain block */
    public int Block;
    //// four slashes
    public int Four;
    int last; //trailing
}
/// <summary>Text.</summary>
public class Empties
{
    /// <summary>Text.</summary>
    /// <remarks></remarks>
    /// <example />
    /// <exception cref="System.Exception"></exception>
    /// <exception cref="System.ArgumentException">Thrown.</exception>
    /// <permission cref="System.Security.PermissionSet" />
    /// <remarks>
    /// </remarks>
    /// <remarks>Text.</remarks>
    /// <returns></returns>
    public int Elements() => 0;
    /// <summary>
    /// Text.
    /// <remarks></remarks>
    /// </summary>
    public void Nested() { }
    /// <remarks></remarks>
    int Hidden;
    /// <remarks><see cref="Hidden"/>.</remarks>
    public int Cref;
    /// <remarks><!-- comment --></remarks>
    public int Comment;
}
