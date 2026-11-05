using System.Threading.Tasks;
/// <summary>Tags.</summary>
public class Values
{
    /// <summary>Gets the value.</summary>
    /// <value></value>
    public int EmptyValue { get; }
//             ^^^^^^^^^^ doc-value-text
    /// <summary>Gets the value.</summary>
    /// <value />
    public int SelfClosingValue { get; }
    /// <summary>Gets the value.</summary>
    /// <value>The value.</value>
    public int GoodValue { get; }
    /// <summary>Gets the value.</summary>
    /// <value>
    /// </value>
    public int BlankValue { get; }
//             ^^^^^^^^^^ doc-value-text
    /// <summary>Gets the value.</summary>
    /// <value></value>
    /// <inheritdoc />
    public int InheritedValue { get; }
    /// <summary>The field.</summary>
    /// <value></value>
    public int Field;
}
/// <summary>Parameters.</summary>
public class Parameters
{
    /// <summary>Sets.</summary>
    /// <param>No name.</param>
//      ^^^^^^^^^^^^^^^^^^^^^^^ doc-param-has-name
    /// <param name="">Empty name.</param>
//             ^^^^^^^ doc-param-has-name
    /// <param name="a"></param>
//      ^^^^^^^^^^^^^^^^^^^^^^^^ doc-param-has-text
    /// <param name="b" />
//      ^^^^^^^^^^^^^^^^^^ doc-param-has-text
    /// <param name="c">Good.</param>
    /// <param name="d">
//      ^ doc-param-has-text
    /// </param>
    public void Params(int a, int b, int c, int d) { }
    /// <summary>Sets.</summary>
    /// <param name="a"></param>
    /// <inheritdoc />
    public void Inherited(int a) { }
    /// <summary>Sets.</summary>
    /// <param name="a"></param>
//      ^^^^^^^^^^^^^^^^^^^^^^^^ doc-param-has-text
    private void PrivateParams(int a) { }
    /// <summary>Delegate.</summary>
    /// <param name="a"></param>
//      ^^^^^^^^^^^^^^^^^^^^^^^^ doc-param-has-text
    /// <param>No name.</param>
//      ^^^^^^^^^^^^^^^^^^^^^^^ doc-param-has-name
    public delegate void Callback(int a);
    /// <summary>Constructor.</summary>
    /// <param name="a"></param>
//      ^^^^^^^^^^^^^^^^^^^^^^^^ doc-param-has-text
    public Parameters(int a) { }
    /// <summary>Indexer.</summary>
    /// <param name="a"></param>
//      ^^^^^^^^^^^^^^^^^^^^^^^^ doc-param-has-text
    /// <param>No name.</param>
//      ^^^^^^^^^^^^^^^^^^^^^^^ doc-param-has-name
    public int this[int a] => a;
}
