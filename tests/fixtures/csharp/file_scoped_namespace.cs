// header comment
extern alias Other;
global using System.Linq;
using System;

[assembly: CLSCompliant(true)]

namespace Company.Product.Feature;

/// <summary>A type in a file-scoped namespace.</summary>
public sealed partial class Widget<T> : IDisposable
    where T : class, new()
{
    private readonly T _value = new();

    public void Dispose() { }
}

internal enum Mode : byte
{
    None = 0,
    Fast,
    Slow = 1 << 3,
}
