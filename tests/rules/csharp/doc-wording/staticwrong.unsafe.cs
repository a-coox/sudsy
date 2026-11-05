using System;
/// <summary>Static.</summary>
public class StaticWrong
{
    /// <summary>Initializes static members of the <see cref="StaticWrong"/> class. Static.</summary>
    static StaticWrong() { }
}
/// <summary>Struct.</summary>
public struct Pair
{
    /// <summary>Initializes a new instance of the <see cref="Pair"/> struct.</summary>
    public Pair(int a) { }
    /// <summary>Initializes a new instance of the <see cref="Pair"/> struct.</summary>
    public Pair(int a, int b) { }
    /// <summary>Prevents a default instance of the <see cref="Pair"/> struct from being created.</summary>
    private Pair(int a, int b, int c) { }
    /// <summary>Initializes static members of the <see cref="Pair"/> struct.</summary>
    static Pair() { }
}
/// <summary>Destructors.</summary>
public class Destructors<T>
{
    /// <summary>Finalizes an instance of the <see cref="Destructors{T}"/> class.</summary>
    ~Destructors() { }
}
/// <summary>Destructors.</summary>
public class DestructorsWrong
{
    /// <summary>Finalizes an instance of the <see cref="DestructorsWrong"/> class. Cleans up.</summary>
    ~DestructorsWrong() { }
}
/// <summary>Destructors.</summary>
public class DestructorsOther
{
    /// <summary>Finalizes an instance of the <see cref="DestructorsOther"/> class.</summary>
    ~DestructorsOther() { }
}
/// <summary>Destructors.</summary>
public class DestructorsMultiLine
{
    /// <summary>
    /// Finalizes an instance of the <see cref="DestructorsMultiLine"/> class.
    /// </summary>
    ~DestructorsMultiLine() { }
}
/// <summary>Destructors.</summary>
public class DestructorsNoPeriod
{
    /// <summary>Finalizes an instance of the <see cref="DestructorsNoPeriod"/> class.</summary>
    ~DestructorsNoPeriod() { }
}
/// <summary>Other.</summary>
public class Other { }
