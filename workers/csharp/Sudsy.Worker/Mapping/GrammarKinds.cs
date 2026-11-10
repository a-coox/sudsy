using System.Runtime.CompilerServices;
using Sudsy.Worker.Generated;
using CS = Microsoft.CodeAnalysis.CSharp;

namespace Sudsy.Worker.Mapping;

/// <summary>Maps Roslyn SyntaxKind values to our grammar kind ids (matched by name).</summary>
public static class GrammarKinds
{
    public const uint Unmapped = uint.MaxValue;
    private const uint TokenBit = 1 << 23;

    /// <summary>The core <c>ERROR</c> node kind (<c>SyntaxKind::ERROR</c> in core-syntax).</summary>
    public const uint Error = 0;

    private static readonly uint[] Table = Build();

    private static uint[] Build()
    {
        var ours = CsharpGrammar.Kinds.ToDictionary(k => k.Name, k => k.Id);
        var table = new uint[Enum.GetValues<CS.SyntaxKind>().Max(v => (int)v) + 1];
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

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static uint Of(int rawRoslynKind)
    {
        var table = Table;
        if ((uint)rawRoslynKind < (uint)table.Length)
        {
            var id = table[rawRoslynKind];
            if (id != Unmapped)
            {
                return id;
            }
        }

        return Fail(rawRoslynKind);
    }

    private static uint Fail(int raw) =>
        throw new MappingException($"Roslyn kind {raw} ({(CS.SyntaxKind)raw}) has no grammar kind");

    public static bool IsToken(uint kind) => (kind & TokenBit) != 0;
}

public sealed class MappingException(string message) : Exception(message);
