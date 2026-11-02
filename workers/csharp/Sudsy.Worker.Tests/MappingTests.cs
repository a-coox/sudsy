using Microsoft.CodeAnalysis.CSharp;
using Sudsy.Worker.Generated;
using Sudsy.Worker.Mapping;
using Xunit;

namespace Sudsy.Worker.Tests;

public class MappingTests
{
    private static string FixtureDir()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null && !Directory.Exists(Path.Combine(dir.FullName, "tests", "fixtures", "csharp")))
        {
            dir = dir.Parent;
        }

        return Path.Combine(dir!.FullName, "tests", "fixtures", "csharp");
    }

    private static FlatTree Map(string text, bool general, params string[] symbols)
    {
        var root = CSharpSyntaxTree.ParseText(text, new CSharpParseOptions(preprocessorSymbols: symbols)).GetRoot();
        return general ? TreeMapper.MapGeneral(root, text) : TreeMapper.Map(root, text);
    }

    private static List<string> Names(FlatTree tree)
    {
        var names = CsharpGrammar.Kinds.ToDictionary(k => k.Id, k => k.Name);
        names[GrammarKinds.Error] = "ERROR";
        var result = new List<string>();
        for (var i = 0; i < tree.Count; i++)
        {
            result.Add($"{names[tree.Kinds[i]]}:{tree.Values[i]}");
        }

        return result;
    }

    [Theory]
    [MemberData(nameof(FixtureFiles))]
    public void TheFastPathAndTheGeneralRoutineAgree(string path)
    {
        var text = File.ReadAllText(path);
        Assert.Equal(Names(Map(text, general: true)), Names(Map(text, general: false)));
        Assert.Equal(Names(Map(text, general: true, "FEATURE", "DEBUG")), Names(Map(text, general: false, "FEATURE", "DEBUG")));
    }

    public static IEnumerable<object[]> FixtureFiles() =>
        Directory.EnumerateFiles(FixtureDir(), "*.cs").Order().Select(f => new object[] { f });

    [Fact]
    public void AMissingTokenIsAZeroWidthErrorNode()
    {
        var names = Names(Map("class A { int x = ; }", general: false));
        var error = names.IndexOf("ERROR:0");
        Assert.True(error > 0);
        Assert.Equal("IdentifierName:1", names[error - 1]);
    }

    [Fact]
    public void SkippedTokensBecomeAnErrorNodeWithTheirTrivia()
    {
        var names = Names(Map("class A { }\nclass B { } @ \n", general: false));
        var error = names.FindIndex(n => n.StartsWith("ERROR:") && n != "ERROR:0");
        Assert.True(error > 0, string.Join(" ", names));
        Assert.Equal("ERROR:3", names[error]);
        Assert.Equal("BadToken:1", names[error + 1]);
        Assert.Equal("WhitespaceTrivia:1", names[error + 2]);
        Assert.Equal("EndOfLineTrivia:1", names[error + 3]);
    }

    [Fact]
    public void DocCommentsAndDirectivesAreNodesAndDisabledTextIsAToken()
    {
        var names = Names(Map("/// <a/>\n#if X\nint a;\n#endif\nclass C { }\n", general: false));
        Assert.Contains(names, n => n.StartsWith("SingleLineDocumentationCommentTrivia:") && !n.EndsWith(":0"));
        Assert.Contains(names, n => n.StartsWith("IfDirectiveTrivia:"));
        Assert.Contains("DisabledTextTrivia:7", names);
        Assert.Contains(names, n => n.StartsWith("EndIfDirectiveTrivia:"));
    }

    [Fact]
    public void PreprocessorSymbolsSelectTheActiveBranch()
    {
        const string text = "#if X\nclass A { }\n#else\nclass B { }\n#endif\n";
        Assert.Contains("DisabledTextTrivia:12", Names(Map(text, general: false, "X")));
        Assert.Contains("DisabledTextTrivia:12", Names(Map(text, general: false)));
    }

    [Fact]
    public void NonAsciiTextGetsByteWidths()
    {
        var names = Names(Map("class Ü { string s = \"é🙂\"; }", general: false));
        Assert.Contains("IdentifierToken:2", names);
        Assert.Contains("StringLiteralToken:8", names);
    }
}
