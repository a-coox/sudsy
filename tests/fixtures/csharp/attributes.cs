using System;
using System.Runtime.InteropServices;

[assembly: AssemblyVersion("1.0.0.0")]
[module: SkipLocalsInit]

[Serializable, Obsolete("old", error: false)]
[StructLayout(LayoutKind.Sequential, Pack = 1)]
public class Attributed
{
    [field: NonSerialized] public int Field;

    [return: MaybeNull]
    [Conditional("DEBUG")]
    public T Method<[DynamicallyAccessedMembers(All)] T>([In, Out] int[] arr, [CallerMemberName] string name = "") => default;

    [property: JsonIgnore]
    public int Prop { [MethodImpl(AggressiveInlining)] get; [Obsolete] set; }

    [Event] public event EventHandler? Changed;
    public Func<int, int> F = [Pure] (x) => x;
}
