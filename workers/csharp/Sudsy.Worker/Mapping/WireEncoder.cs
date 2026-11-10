using Sudsy.Worker.Protocol;

namespace Sudsy.Worker.Mapping;

/// <summary>Serializes a <see cref="FlatTree"/>: kinds as little-endian u32, values as LEB128 varints.</summary>
public static class WireEncoder
{
    public static WireTree Encode(FlatTree tree)
    {
        var kinds = new byte[tree.Count * 4];
        var values = new byte[tree.Count * 5];
        var used = 0;
        var k = tree.Kinds;
        var v = tree.Values;
        for (var i = 0; i < k.Length; i++)
        {
            System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(kinds.AsSpan(4 * i), k[i]);
            used += WriteVarint(values.AsSpan(used), v[i]);
        }

        return new WireTree
        {
            Version = ProtocolInfo.WireFormatVersion,
            Kinds = kinds,
            Values = values.AsSpan(0, used).ToArray(),
        };
    }

    /// <summary>Inverse of <see cref="Encode"/>; used by tests.</summary>
    public static FlatTree Decode(WireTree wire)
    {
        var tree = new FlatTree();
        var pos = 0;
        for (var i = 0; i < wire.Kinds.Length / 4; i++)
        {
            var value = 0u;
            var shift = 0;
            byte b;
            do
            {
                b = wire.Values[pos++];
                value |= (uint)(b & 0x7f) << shift;
                shift += 7;
            }
            while ((b & 0x80) != 0);
            tree.Add(System.Buffers.Binary.BinaryPrimitives.ReadUInt32LittleEndian(wire.Kinds.AsSpan(4 * i)), value);
        }

        return tree;
    }

    private static int WriteVarint(Span<byte> dest, uint value)
    {
        var n = 0;
        while (value >= 0x80)
        {
            dest[n++] = (byte)(value | 0x80);
            value >>= 7;
        }

        dest[n++] = (byte)value;
        return n;
    }
}
