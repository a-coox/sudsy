using System;
/// <summary>Constructors.</summary>
public class Constructors<T>
{
    /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class.</summary>
    public Constructors() { }
    /// <summary>Creates it.</summary>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ constructor-summary-standard-text
    public Constructors(int a) { }
    /// <summary>Initializes a new instance of the <see cref="Other"/> class.</summary>
//                                                 ^^^^^^^^^^^^^^^^^^^ constructor-summary-standard-text
    public Constructors(int a, int b) { }
    /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> struct.</summary>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ constructor-summary-standard-text
    public Constructors(int a, int b, int c) { }
    /// <summary>Initializes a new instance of the Constructors class.</summary>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ constructor-summary-standard-text
    public Constructors(long a) { }
    /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class and more.</summary>
    public Constructors(long a, long b) { }
    /// <summary>
    /// Initializes a new instance of the <see cref="Constructors{T}"/> class.
    /// </summary>
    public Constructors(string a) { }
    /// <summary>
    /// Initializes a new instance of the
    /// <see cref="Constructors{T}"/> class.
    /// </summary>
    public Constructors(string a, string b) { }
    /// <summary>
    /// Initializes  a new instance of the <see cref="Constructors{T}"/>   class.
    /// </summary>
    public Constructors(string a, string b, string c) { }
    /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class.</summary>
    /// <remarks>More.</remarks>
    internal Constructors(char a) { }
    /// <summary>Initializes static members of the <see cref="Constructors{T}"/> class.</summary>
    static Constructors() { }
    /// <summary>Prevents a default instance of the <see cref="Constructors{T}"/> class from being created.</summary>
    private Constructors(char a, char b) { }
    /// <summary>Initializes a new instance of the <see cref="Constructors{T}"/> class.</summary>
    private Constructors(char a, char b, char c) { }
    /// <summary>Private.</summary>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^ constructor-summary-standard-text
    private Constructors(char a, char b, char c, char d) { }
    /// <summary>Prevents a default instance of the <see cref="Constructors{T}"/> class from being created.</summary>
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ constructor-summary-standard-text
    protected Constructors(byte a) { }
    /// <summary>Finalizes an instance of the <see cref="Constructors{T}"/> class.</summary>
    ~Constructors() { }
    /// <inheritdoc />
    protected Constructors(byte a, byte b) { }
    /// <summary />
    protected Constructors(byte a, byte b, byte c) { }
    /// <remarks>No summary.</remarks>
    protected Constructors(byte a, byte b, byte c, byte d) { }
}
