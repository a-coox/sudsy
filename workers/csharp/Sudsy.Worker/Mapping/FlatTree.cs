namespace Sudsy.Worker.Mapping;

/// <summary>
/// A tree as two parallel pre-order arrays: the element kind and one number
/// (token: width in UTF-8 bytes; node: child count). See the wire encoding in
/// crates/core-syntax/src/wire.rs.
/// </summary>
public sealed class FlatTree
{
    private ushort[] _kinds = new ushort[256];
    private uint[] _values = new uint[256];

    public int Count { get; private set; }

    public ReadOnlySpan<ushort> Kinds => _kinds.AsSpan(0, Count);

    public ReadOnlySpan<uint> Values => _values.AsSpan(0, Count);

    /// <summary>Appends an element and returns its slot.</summary>
    public int Add(ushort kind, uint value)
    {
        if (Count == _kinds.Length)
        {
            Array.Resize(ref _kinds, Count * 2);
            Array.Resize(ref _values, Count * 2);
        }

        _kinds[Count] = kind;
        _values[Count] = value;
        return Count++;
    }

    public void SetValue(int slot, uint value) => _values[slot] = value;
}
