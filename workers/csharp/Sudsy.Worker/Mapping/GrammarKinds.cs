using Sudsy.Worker.Generated;
using CS = Microsoft.CodeAnalysis.CSharp;

namespace Sudsy.Worker.Mapping;

/// <summary>Maps Roslyn SyntaxKind values to our grammar kind ids (matched by name).</summary>
public static class GrammarKinds
{
    public const ushort Unmapped = ushort.MaxValue;
    private const ushort TokenBit = 1 << 11;

    private static readonly ushort[] Table = Build();

    private static ushort[] Build()
    {
        var ours = CsharpGrammar.Kinds.ToDictionary(k => k.Name, k => k.Id);
        var table = new ushort[Enum.GetValues<CS.SyntaxKind>().Max(v => (int)v) + 1];
        Array.Fill(table, Unmapped);
        foreach (var name in Enum.GetNames<CS.SyntaxKind>())
        {
            if (ours.TryGetValue(name, out var id))
            {
                table[(int)Enum.Parse<CS.SyntaxKind>(name)] = id;
            }
        }

        return table;
    }

    public static ushort Of(int rawRoslynKind) =>
        (uint)rawRoslynKind < (uint)Table.Length && Table[rawRoslynKind] != Unmapped
            ? Table[rawRoslynKind]
            : throw new MappingException($"Roslyn kind {rawRoslynKind} ({(CS.SyntaxKind)rawRoslynKind}) has no grammar kind");

    public static bool IsToken(ushort kind) => (kind & TokenBit) != 0;
}

public sealed class MappingException(string message) : Exception(message);
