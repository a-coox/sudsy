using System.Threading.Tasks;

namespace DocTags
{
    /// <summary>Tags.</summary>
    public class Values
    {
        /// <summary>Gets the value.</summary>
        /// <value></value>
        public int EmptyValue { get; }

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
        /// <param name="">Empty name.</param>
        /// <param name="a"></param>
        /// <param name="b" />
        /// <param name="c">Good.</param>
        /// <param name="d">
        /// </param>
        public void Params(int a, int b, int c, int d) { }

        /// <summary>Sets.</summary>
        /// <param name="a"></param>
        /// <inheritdoc />
        public void Inherited(int a) { }

        /// <summary>Sets.</summary>
        /// <param name="a"></param>
        private void PrivateParams(int a) { }

        /// <summary>Delegate.</summary>
        /// <param name="a"></param>
        /// <param>No name.</param>
        public delegate void Callback(int a);

        /// <summary>Constructor.</summary>
        /// <param name="a"></param>
        public Parameters(int a) { }

        /// <summary>Indexer.</summary>
        /// <param name="a"></param>
        /// <param>No name.</param>
        public int this[int a] => a;
    }

    /// <summary>Return values.</summary>
    public class Returns
    {
        /// <summary>Returns.</summary>
        public int NoReturns() => 0;

        /// <summary>Returns.</summary>
        /// <returns></returns>
        public int EmptyReturns() => 0;

        /// <summary>Returns.</summary>
        /// <returns />
        public int SelfClosingReturns() => 0;

        /// <summary>Returns.</summary>
        /// <returns>The value.</returns>
        public int GoodReturns() => 0;

        /// <summary>Returns.</summary>
        /// <returns>
        /// </returns>
        public int BlankReturns() => 0;

        /// <summary>Does.</summary>
        /// <returns>Nothing.</returns>
        public void VoidReturns() { }

        /// <summary>Does.</summary>
        /// <returns></returns>
        public void VoidEmptyReturns() { }

        /// <summary>Does.</summary>
        /// <returns />
        public void VoidSelfClosingReturns() { }

        /// <summary>Does.</summary>
        public Task AsyncNoReturns() => null;

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

        /// <summary>Does.</summary>
        protected int ProtectedNoReturns() => 0;

        /// <summary>Does.</summary>
        public static int operator +(Returns a, Returns b) => 0;

        /// <summary>Delegate.</summary>
        public delegate int Callback();

        /// <summary>Delegate.</summary>
        /// <returns>Nothing.</returns>
        public delegate void VoidCallback();

        /// <summary>Does.</summary>
        public ref int RefReturn() => throw null;
    }

    /// <summary>Generic.</summary>
    /// <typeparam>No name.</typeparam>
    /// <typeparam name="">Empty name.</typeparam>
    /// <typeparam name="T"></typeparam>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
    /// <typeparam name="U" />
//      ^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
    /// <typeparam name="V">Good.</typeparam>
    public class Generic<T, U, V>
    {
        /// <summary>Method.</summary>
        /// <typeparam name="X"></typeparam>
//          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
        /// <typeparam>No name.</typeparam>
        public void Method<X>() { }

        /// <summary>Method.</summary>
        /// <typeparam name="X"></typeparam>
        /// <inheritdoc />
        public void Inherited<X>() { }

        /// <summary>Not generic.</summary>
        /// <typeparam name="X"></typeparam>
        public void NotGeneric() { }
    }

    /// <summary>Struct.</summary>
    /// <typeparam name="T"></typeparam>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
    public struct GenericStruct<T> { }

    /// <summary>Interface.</summary>
    /// <typeparam>T</typeparam>
    public interface IGeneric<T> { }

    /// <summary>Delegate.</summary>
    /// <typeparam name="T"></typeparam>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
    /// <typeparam name="U" />
//      ^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
    public delegate void GenericCallback<T, U>();

    /// <summary>Partial.</summary>
    /// <typeparam name="T">The T.</typeparam>
    public partial class PartialGeneric<T, U> { }

    /// <summary>Partial.</summary>
    public partial class PartialGenericNone<T> { }

    /// <summary>Partial.</summary>
    /// <typeparam name="U">The U.</typeparam>
    internal partial struct PartialGenericStruct<T, U> { }

    /// <summary>Partial.</summary>
    /// <typeparam name="T">The T.</typeparam>
    /// <typeparam name="U">The U.</typeparam>
    public partial class PartialGenericGood<T, U> { }

    /// <remarks>Partial without summary.</remarks>
    public partial class PartialGenericNoSummary<T> { }

    /// <inheritdoc />
    public partial class PartialGenericInherited<T> { }

    /// <summary>Partial, no generics.</summary>
    public partial class PartialPlain { }
}
