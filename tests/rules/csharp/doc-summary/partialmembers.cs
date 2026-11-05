/// <summary>Part one.</summary>
public partial class PartialMembers
{
    public partial void Declared();
//                      ^^^^^^^^ doc-partial-required
    partial void Private();
    /// <summary>Documented.</summary>
    public partial void Implemented();
    /// <remarks>Without summary.</remarks>
    public partial void NoSummary();
//                      ^^^^^^^^^ doc-partial-summary-required
    /// <summary></summary>
    public partial void EmptySummary();
//                      ^^^^^^^^^^^^ doc-partial-required
//                      ^^^^^^^^^^^^ doc-partial-summary-text
}
/// <summary>Part two.</summary>
public partial class PartialMembers
{
    public partial void Declared() { }
//                      ^^^^^^^^ doc-partial-required
    partial void Private() { }
    /// <summary>Documented.</summary>
    public partial void Implemented() { }
    /// <remarks>Without summary.</remarks>
    public partial void NoSummary() { }
//                      ^^^^^^^^^ doc-partial-summary-required
    /// <summary></summary>
    public partial void EmptySummary() { }
//                      ^^^^^^^^^^^^ doc-partial-required
//                      ^^^^^^^^^^^^ doc-partial-summary-text
}
