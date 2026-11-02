namespace Sudsy.Worker.Mapping;

/// <summary>A tree as parallel arrays in pre-order: per element its kind and one number (token: UTF-8 width, node: child count).</summary>
public sealed class FlatTree(int capacity = 256)
{
    private ushort[] _kinds = new ushort[Math.Max(capacity, 16)];
    private uint[] _values = new uint[Math.Max(capacity, 16)];

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
