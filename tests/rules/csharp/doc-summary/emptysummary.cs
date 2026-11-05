/// <summary></summary>
public class EmptySummary
//           ^^^^^^^^^^^^ doc-summary-text
{
    /// <summary>
    /// </summary>
    public void Blank() { }
//              ^^^^^ doc-summary-text
    /// <summary />
    public void SelfClosing() { }
    /// <summary><see cref="Blank"/></summary>
    public void OnlyCref() { }
    /// <summary>
    /// <!-- comment -->
    /// </summary>
    public void OnlyComment() { }
//              ^^^^^^^^^^^ doc-summary-text
    /// <summary>   </summary>
    public int Field;
//             ^^^^^ doc-summary-text
    /// <summary></summary>
    public int First, Second;
//             ^^^^^ doc-summary-text
//                    ^^^^^^ doc-summary-text
}
/// <summary>Summary description for Defaults.</summary>
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-default-summary
public class Defaults
{
    /// <summary>
//      ^ no-default-summary
    /// Summary description for Method.
    /// </summary>
    public void Method() { }
    /// <summary><see cref="Method"/> Summary description for x</summary>
    public void NotFirst() { }
    /// <summary>summary description for lower case</summary>
    public void LowerCase() { }
    /// <summary>
//      ^ no-default-summary
    ///   Summary description for indented.
    /// </summary>
    public int Property { get; }
    /// <summary>Summary description for.</summary>
    /// <inheritdoc />
    public override string ToString() => "";
}
/// <remarks>Partial without summary.</remarks>
public partial class PartialNoSummary { }
//                   ^^^^^^^^^^^^^^^^ doc-partial-summary-required
/// <summary></summary>
public partial class PartialEmptySummary { }
//                   ^^^^^^^^^^^^^^^^^^^ doc-partial-required
//                   ^^^^^^^^^^^^^^^^^^^ doc-partial-summary-text
/// <content>Content.</content>
public partial class PartialContent { }
/// <content></content>
public partial class PartialEmptyContent { }
//                   ^^^^^^^^^^^^^^^^^^^ doc-partial-required
//                   ^^^^^^^^^^^^^^^^^^^ doc-partial-summary-text
/// <inheritdoc />
public partial class PartialInherited { }
/// <summary>Summary description for PartialDefault.</summary>
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-default-summary
public partial class PartialDefault { }
/// <remarks>Internal partial.</remarks>
internal partial struct PartialInternal { }
//                      ^^^^^^^^^^^^^^^ doc-partial-summary-required
public partial class PartialUndocumented { }
//                   ^^^^^^^^^^^^^^^^^^^ doc-partial-required
internal partial struct PartialUndocumentedInternal { }
//                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-partial-required
