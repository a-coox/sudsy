using System.Text;
using MessagePack;
using Sudsy.Worker.Generated;
using Sudsy.Worker.Protocol;
using Xunit;

namespace Sudsy.Worker.Tests;

public class SemanticTests
{
    private const string Source = """
        using System;
        using System.Text;
        using System.Diagnostics.CodeAnalysis;
        class C {
          int F(int a) {
            int unused;
            int k = 5;
            int z = G();
            try { G(); } catch (Exception e) { }
            if (false) { G(); }
            while (true) { if (a > 0) break; }
            return z;
            G();
            G();
          }
          int G() => 1;
          private int hidden;
          [DoesNotReturn] static void Fail() => throw new Exception();
          async System.Threading.Tasks.Task<int> Load() => 1;
        }
        """;

    private static Hello Hello() => new()
    {
        ProtocolVersion = ProtocolInfo.Version,
        GrammarHash = CsharpGrammar.Hash,
        FlowGraphFormat = ProtocolInfo.FlowGraphFormatVersion,
        WireFormat = ProtocolInfo.WireFormatVersion,
    };

    private static List<(string Variant, byte[] Frame)> Converse(params byte[][] requests)
    {
        var input = new MemoryStream();
        Envelope.WriteFrame(input, Envelope.Encode("Hello", Hello()));
        foreach (var r in requests)
        {
            Envelope.WriteFrame(input, r);
        }

        input.Position = 0;
        var output = new MemoryStream();
        new WorkerServer(input, output).Run();
        output.Position = 0;
        var responses = new List<(string, byte[])>();
        while (Envelope.ReadFrame(output) is { } frame)
        {
            var reader = new MessagePackReader(frame);
            responses.Add((Envelope.ReadVariant(ref reader), frame));
        }

        return responses.Skip(1).ToList();
    }

    private static T Payload<T>(byte[] frame)
    {
        var reader = new MessagePackReader(frame);
        Envelope.ReadVariant(ref reader);
        return Envelope.Payload<T>(ref reader);
    }

    private static byte[] Update(string text = Source) =>
        Envelope.Encode("UpdateText", new UpdateText { FileId = "t.cs", Version = 1, Text = text });

    [Fact]
    public void QueryBeforeAnyTextIsABadRequest()
    {
        var r = Converse(Envelope.Encode("Query", new Query { FileId = "nope.cs", Items = ["declarations"] }));
        Assert.Equal("Error", r[0].Variant);
        Assert.Equal("bad_request", Payload<ProtocolError>(r[0].Frame).Code);
    }

    [Fact]
    public void CompilerDiagnosticsAreReportedInUtf8Offsets()
    {
        var text = "class C { void M() { var s = \"é\"; int k = 5; } }";
        var r = Converse(Update(text), Envelope.Encode("Query", new Query
        {
            FileId = "t.cs",
            Version = 1,
            Items = ["diagnostics"],
            DiagnosticIds = ["CS0219"],
        }));
        var answers = Payload<Answers>(r[1].Frame);
        var d = answers.Diagnostics.Single(x => x.Message.Contains("'k'"));
        Assert.Equal("CS0219", d.Id);
        // 'é' is two bytes: the span starts one byte later than its UTF-16 index.
        Assert.Equal(Encoding.UTF8.GetBytes(text[..text.IndexOf("k = 5", StringComparison.Ordinal)]).Length, (int)d.Start);
    }

    [Fact]
    public void ExportedGraphsKeepUnreachableBlocksAndConstants()
    {
        var r = Converse(Update(), Envelope.Encode("FlowGraphs", new FlowGraphs { FileId = "t.cs", Version = 1 }));
        var graphs = Payload<Graphs>(r[1].Frame);
        var f = graphs.Bodies.Single(b => b.Symbol.StartsWith("M:C.F("));
        Assert.Contains(f.Locals, l => l.Name == "unused" && l.Kind == 0);
        Assert.Contains(f.Locals, l => l.Name == "e" && l.Kind == 1);
        Assert.Contains(f.Blocks, b => b.BranchConst == false);
        // The two calls after the return sit in a block no edge leads to.
        var targets = f.Blocks.SelectMany(b => b.Succ.Select(e => e.To)).ToHashSet();
        var dead = f.Blocks.Select((b, i) => (b, i)).Where(p => p.i > 0 && !targets.Contains((uint)p.i) && p.b.Ops.Length > 0).ToList();
        Assert.Contains(dead, p => p.b.Ops.Count(o => o.Kind == 4) == 2);
    }

    [Fact]
    public void DeclarationsCarryTheFlagsRulesNeed()
    {
        var r = Converse(Update(), Envelope.Encode("Query", new Query { FileId = "t.cs", Version = 1, Items = ["declarations"] }));
        var decls = Payload<Answers>(r[1].Frame).Declarations;
        var hidden = decls.Single(d => d.Name == "hidden");
        Assert.Equal((byte)2, hidden.Kind);
        Assert.Equal((byte)0, hidden.Accessibility);
        var load = decls.Single(d => d.Name == "Load");
        Assert.Equal(1 | 64, load.Flags & (1 | 64));
        Assert.NotEqual(0, decls.Single(d => d.Name == "Fail").Flags & 16);
    }

    [Fact]
    public void UnusedImportsAreWithheldWhileTheFileHasErrors()
    {
        var ok = "using System;\nusing System.Text;\nclass C { object o = new Exception(); }";
        var r = Converse(Update(ok), Envelope.Encode("Query", new Query { FileId = "t.cs", Version = 1, Items = ["unused_imports"] }));
        var answers = Payload<Answers>(r[1].Frame);
        var unused = Assert.Single(answers.UnusedImports);
        Assert.Equal(ok.IndexOf("using System.Text;", StringComparison.Ordinal), (int)unused.Start);

        var broken = ok + "\nclass D { Missing m; }";
        r = Converse(Update(broken), Envelope.Encode("Query", new Query { FileId = "t.cs", Version = 1, Items = ["unused_imports"] }));
        answers = Payload<Answers>(r[1].Frame);
        Assert.Empty(answers.UnusedImports);
        Assert.True(answers.SemanticErrors > 0);
    }

    [Fact]
    public void AnEditReplacesTheTextTheNextQuerySees()
    {
        var r = Converse(
            Update("class A { int k() { int x = 1; return 0; } }"),
            Update("class A { int k() { return 0; } }"),
            Envelope.Encode("Query", new Query { FileId = "t.cs", Version = 1, Items = ["diagnostics"], DiagnosticIds = ["CS0219"] }));
        Assert.Empty(Payload<Answers>(r[2].Frame).Diagnostics);
    }

    [Fact]
    public void MaybeNullReceiversComeFromTheNullableAnalysis()
    {
        var text = "#nullable enable\nclass A { string? s; int N() => s.Length; int M() => s!.Length; }";
        var r = Converse(Update(text), Envelope.Encode("Query", new Query { FileId = "t.cs", Version = 1, Items = ["maybe_null"] }));
        var spans = Payload<Answers>(r[1].Frame).MaybeNull;
        var span = Assert.Single(spans);
        Assert.Equal(text.IndexOf("s.Length", StringComparison.Ordinal), (int)span.Start);
    }
}

public class FixtureExportTests
{
    [Fact]
    public void TheFlowAgreementFixtureExportsWithoutNullStrings()
    {
        var root = AppContext.BaseDirectory;
        while (!File.Exists(Path.Combine(root, "Cargo.toml")))
        {
            root = Path.GetDirectoryName(root)!;
        }

        var text = File.ReadAllText(Path.Combine(root, "crates/lang-csharp/tests/data/flow-agreement.cs"));
        var workspace = new Sudsy.Worker.Semantics.Workspace();
        workspace.Update("fixture.cs", 1, text);
        var (document, model) = workspace.Get("fixture.cs")!.Value;
        var bodies = Sudsy.Worker.Semantics.FlowExporter.Export(document, model);
        Assert.True(bodies.Length > 30);
        var bytes = MessagePack.MessagePackSerializer.Serialize(new Sudsy.Worker.Protocol.Graphs { Bodies = bodies });
        Assert.All(bodies, b => Assert.NotNull(b.Symbol));
        Assert.All(bodies.SelectMany(b => b.Locals), l => Assert.NotNull(l.Name));
        Assert.All(bodies.SelectMany(b => b.Blocks).SelectMany(bl => bl.Ops), o => Assert.NotNull(o.Callee));
        Assert.NotEmpty(bytes);
    }
}

public class WorkspaceLimitsTests
{
    [Fact]
    public void OpeningARootSkipsLinksOversizedFilesAndSolutionProjectsOutsideIt()
    {
        var top = Directory.CreateTempSubdirectory("sudsy-ws").FullName;
        try
        {
            var root = Path.Combine(top, "repo");
            var outside = Path.Combine(top, "outside");
            Directory.CreateDirectory(root);
            Directory.CreateDirectory(outside);
            File.WriteAllText(Path.Combine(root, "ok.cs"), "class Ok {}");
            File.WriteAllText(Path.Combine(root, "big.cs"), "//" + new string('x', (int)Sudsy.Worker.Semantics.Workspace.MaxFileBytes));
            File.WriteAllText(Path.Combine(outside, "out.cs"), "class Out {}");

            try
            {
                Directory.CreateSymbolicLink(Path.Combine(root, "link"), outside);
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                // Symlinks need privileges on Windows; the test then covers the other limits only.
            }

            var workspace = new Sudsy.Worker.Semantics.Workspace();
            workspace.Open([root]);
            var names = workspace.Documents.Select(d => Path.GetFileName(d.Id)).ToList();
            _ = workspace.Get(Path.Combine(root, "ok.cs"));
            names = workspace.Documents.Select(d => Path.GetFileName(d.Id)).ToList();
            Assert.Contains("ok.cs", names);
            Assert.DoesNotContain("big.cs", names);
            Assert.DoesNotContain("out.cs", names);


            File.WriteAllText(Path.Combine(root, "x.sln"), "Project(\"{1}\") = \"x\", \"..\\outside\\x.csproj\", \"{2}\"");
            File.WriteAllText(Path.Combine(outside, "x.csproj"), "<Project/>");
            workspace.Open([Path.Combine(root, "x.sln")]);
            _ = workspace.Get(Path.Combine(root, "ok.cs"));
            Assert.DoesNotContain("out.cs", workspace.Documents.Select(d => Path.GetFileName(d.Id)));
        }
        finally
        {
            Directory.Delete(top, true);
        }
    }
}
