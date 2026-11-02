using MessagePack;

namespace Sudsy.Worker.Protocol;

/// <summary>
/// Frames are a 4-byte little-endian length followed by a MessagePack message. A message is an
/// externally tagged enum, as rmp-serde writes it: a one-entry map from the variant name to the payload.
/// </summary>
public static class Envelope
{
    public const int MaxFrameBytes = 512 * 1024 * 1024;

    /// <summary>Reads one frame; null at a clean end of stream.</summary>
    public static byte[]? ReadFrame(Stream input)
    {
        Span<byte> header = stackalloc byte[4];
        var read = input.ReadAtLeast(header, 4, throwOnEndOfStream: false);
        if (read == 0)
        {
            return null;
        }

        if (read < 4)
        {
            throw new EndOfStreamException("truncated frame header");
        }

        var length = BitConverter.ToUInt32(header);
        if (length > MaxFrameBytes)
        {
            throw new InvalidDataException($"frame of {length} bytes exceeds the limit");
        }

        var payload = new byte[length];
        input.ReadExactly(payload);
        return payload;
    }

    public static void WriteFrame(Stream output, ReadOnlySpan<byte> payload)
    {
        output.Write(BitConverter.GetBytes((uint)payload.Length));
        output.Write(payload);
        output.Flush();
    }

    /// <summary>Returns the variant name and the reader positioned at its payload.</summary>
    public static string ReadVariant(ref MessagePackReader reader)
    {
        if (reader.ReadMapHeader() != 1)
        {
            throw new InvalidDataException("expected a one-entry map");
        }

        return reader.ReadString() ?? throw new InvalidDataException("null variant name");
    }

    public static byte[] Encode<T>(string variant, T payload)
    {
        var buffer = new System.Buffers.ArrayBufferWriter<byte>();
        var writer = new MessagePackWriter(buffer);
        writer.WriteMapHeader(1);
        writer.Write(variant);
        MessagePackSerializer.Serialize(ref writer, payload);
        writer.Flush();
        return buffer.WrittenMemory.ToArray();
    }

    public static T Payload<T>(ref MessagePackReader reader) => MessagePackSerializer.Deserialize<T>(ref reader);
}
