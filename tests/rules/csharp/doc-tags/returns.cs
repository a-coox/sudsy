using System.Threading.Tasks;
/// <summary>Return values.</summary>
public class Returns
{
    /// <summary>Returns.</summary>
    public int NoReturns() => 0;
//         ^^^ doc-returns-required
    /// <summary>Returns.</summary>
    /// <returns></returns>
//      ^^^^^^^^^^^^^^^^^^^ doc-returns-has-text
    public int EmptyReturns() => 0;
    /// <summary>Returns.</summary>
    /// <returns />
//      ^^^^^^^^^^^ doc-returns-has-text
    public int SelfClosingReturns() => 0;
    /// <summary>Returns.</summary>
    /// <returns>The value.</returns>
    public int GoodReturns() => 0;
    /// <summary>Returns.</summary>
    /// <returns>
//      ^ doc-returns-has-text
    /// </returns>
    public int BlankReturns() => 0;
    /// <summary>Does.</summary>
    /// <returns>Nothing.</returns>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-returns-on-void
    public void VoidReturns() { }
    /// <summary>Does.</summary>
    /// <returns></returns>
//      ^^^^^^^^^^^^^^^^^^^ doc-returns-has-text
//      ^^^^^^^^^^^^^^^^^^^ no-returns-on-void
    public void VoidEmptyReturns() { }
    /// <summary>Does.</summary>
    /// <returns />
//      ^^^^^^^^^^^ doc-returns-has-text
//      ^^^^^^^^^^^ no-returns-on-void
    public void VoidSelfClosingReturns() { }
    /// <summary>Does.</summary>
    public Task AsyncNoReturns() => null;
//         ^^^^ doc-returns-required
    /// <summary>Does.</summary>
    /// <inheritdoc />
    public int Inherited() => 0;
    /// <summary>Does.</summary>
    /// <returns></returns>
    /// <inheritdoc />
    public int InheritedEmpty() => 0;
    /// <summary>Does.</summary>
    private int PrivateNoReturns() => 0;
    /// <summary>Does.</summary>
    internal int InternalNoReturns() => 0;
//           ^^^ doc-returns-required
    /// <summary>Does.</summary>
    protected int ProtectedNoReturns() => 0;
//            ^^^ doc-returns-required
    /// <summary>Does.</summary>
    public static int operator +(Returns a, Returns b) => 0;
    /// <summary>Delegate.</summary>
    public delegate int Callback();
//                  ^^^ doc-returns-required
    /// <summary>Delegate.</summary>
    /// <returns>Nothing.</returns>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-returns-on-void
    public delegate void VoidCallback();
    /// <summary>Does.</summary>
    public ref int RefReturn() => throw null;
//         ^^^^^^^ doc-returns-required
}
