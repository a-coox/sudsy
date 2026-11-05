using System.Threading.Tasks;
/// <summary>Generic.</summary>
/// <typeparam>No name.</typeparam>
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-name
/// <typeparam name="">Empty name.</typeparam>
//                   ^ doc-typeparam-has-name
/// <typeparam name="T"></typeparam>
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
/// <typeparam name="U" />
//  ^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
/// <typeparam name="V">Good.</typeparam>
public class Generic<T, U, V>
{
    /// <summary>Method.</summary>
    /// <typeparam name="X"></typeparam>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
    /// <typeparam>No name.</typeparam>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-name
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
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
public struct GenericStruct<T> { }
/// <summary>Interface.</summary>
/// <typeparam>T</typeparam>
//  ^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-name
public interface IGeneric<T> { }
/// <summary>Delegate.</summary>
/// <typeparam name="T"></typeparam>
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
/// <typeparam name="U" />
//  ^^^^^^^^^^^^^^^^^^^^^^ doc-typeparam-has-text
public delegate void GenericCallback<T, U>();
/// <summary>Partial.</summary>
/// <typeparam name="T">The T.</typeparam>
public partial class PartialGeneric<T, U> { }
//                                     ^ doc-partial-typeparams
/// <summary>Partial.</summary>
public partial class PartialGenericNone<T> { }
//                                      ^ doc-partial-typeparams
/// <summary>Partial.</summary>
/// <typeparam name="U">The U.</typeparam>
internal partial struct PartialGenericStruct<T, U> { }
//                                           ^ doc-partial-typeparams
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
