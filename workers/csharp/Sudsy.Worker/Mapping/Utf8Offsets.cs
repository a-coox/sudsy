using Microsoft.CodeAnalysis.Text;

namespace Sudsy.Worker.Mapping;

/// <summary>Converts Roslyn's UTF-16 offsets into the UTF-8 byte offsets the core uses.</summary>
internal sealed class Utf8Offsets
{
    private readonly int[]? _offsets;

    public Utf8Offsets(string text)
    {
        _offsets = System.Text.Ascii.IsValid(text) ? null : Build(text);
    }

    public uint At(int index) => _offsets is null ? (uint)index : (uint)_offsets[index];

    public uint Width(TextSpan span) => At(span.End) - At(span.Start);

    private static int[] Build(string text)
    {
        var offsets = new int[text.Length + 1];
        var bytes = 0;
        for (var i = 0; i < text.Length; i++)
        {
            var c = text[i];
            if (c < 0x80)
            {
                bytes += 1;
            }
            else if (c < 0x800)
            {
                bytes += 2;
            }
            else if (char.IsHighSurrogate(c) && i + 1 < text.Length && char.IsLowSurrogate(text[i + 1]))
            {
                bytes += 4;
                offsets[i + 1] = bytes;
                i++;
            }
            else
            {
                bytes += 3;
            }

            offsets[i + 1] = bytes;
        }

        return offsets;
    }
}
