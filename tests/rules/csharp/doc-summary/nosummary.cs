/// <remarks>No summary.</remarks>
public class NoSummary
//           ^^^^^^^^^ doc-summary-required
{
    /// <remarks>No summary.</remarks>
    public int Field;
//             ^^^^^ doc-summary-required
    /// <remarks>No summary.</remarks>
    public int First, Second;
//             ^^^^^ doc-summary-required
//                    ^^^^^^ doc-summary-required
    /// <remarks>No summary.</remarks>
    public NoSummary() { }
//         ^^^^^^^^^ doc-summary-required
    /// <remarks>No summary.</remarks>
    ~NoSummary() { }
//   ^^^^^^^^^ doc-summary-required
    /// <remarks>No summary.</remarks>
    public int this[int index] => index;
//             ^^^^ doc-summary-required
    /// <remarks>No summary.</remarks>
    public event System.EventHandler Changed;
//                                   ^^^^^^^ doc-summary-required
    /// <remarks>No summary.</remarks>
    public delegate void Callback();
//                       ^^^^^^^^ doc-summary-required
    /// <remarks>No summary.</remarks>
    public enum Kind
//              ^^^^ doc-summary-required
    {
        /// <remarks>No summary.</remarks>
        A,
    }
    /// <remarks>No summary.</remarks>
    private int hidden;
    /// <remarks>No summary.</remarks>
    internal int Internal;
//               ^^^^^^^^ doc-summary-required
    /// <remarks>No summary.</remarks>
    protected int Protected;
//                ^^^^^^^^^ doc-summary-required
    /// <remarks>No summary.</remarks>
    private class PrivateType
    {
        /// <remarks>No summary.</remarks>
        public int Field;
    }
    /// <remarks>No summary.</remarks>
    public interface IInterface
//                   ^^^^^^^^^^ doc-summary-required
    {
        /// <remarks>No summary.</remarks>
        void Run();
//           ^^^ doc-summary-required
    }
}
/// <remarks>No summary, internal.</remarks>
internal class InternalNoSummary { }
//             ^^^^^^^^^^^^^^^^^ doc-summary-required
/// <remarks>No summary, private members.</remarks>
internal class InternalHolder
//             ^^^^^^^^^^^^^^ doc-summary-required
{
    /// <remarks>No summary.</remarks>
    public int Field;
//             ^^^^^ doc-summary-required
}
/// <inheritdoc />
public class Inherited : NoSummary { }
