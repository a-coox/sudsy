using System.Text;
using MessagePack;
using Sudsy.Worker.Generated;
using Sudsy.Worker.Mapping;
using Sudsy.Worker.Protocol;
using Xunit;

namespace Sudsy.Worker.Tests;

public class WorkerContractTests
{
    private static Hello ValidHello() => new()
    {
        ProtocolVersion = ProtocolInfo.Version,
        GrammarHash = CsharpGrammar.Hash,
        FlowGraphFormat = ProtocolInfo.FlowGraphFormatVersion,
        WireFormat = ProtocolInfo.WireFormatVersion,
    };

    /// <summary>Feeds the requests to a fresh server and returns (variant, raw payload frame) per response plus the exit code.</summary>
    private static (List<(string Variant, byte[] Frame)> Responses, int ExitCode) Converse(params byte[][] requests)
    {
        var input = new MemoryStream();
        foreach (var r in requests)
        {
            Envelope.WriteFrame(input, r);
        }

        input.Position = 0;
        var output = new MemoryStream();
        var exit = new WorkerServer(input, output).Run();
        output.Position = 0;
        var responses = new List<(string, byte[])>();
        while (Envelope.ReadFrame(output) is { } frame)
        {
            var reader = new MessagePackReader(frame);
            responses.Add((Envelope.ReadVariant(ref reader), frame));
        }

        return (responses, exit);
    }

    private static T Payload<T>(byte[] frame)
    {
        var reader = new MessagePackReader(frame);
        Envelope.ReadVariant(ref reader);
        return Envelope.Payload<T>(ref reader);
    }

    private static byte[] ParseRequest(string text, string? languageVersion = null) =>
        Envelope.Encode("Parse", new ParseRequest { FileId = "a.cs", Version = 7, Text = text, LanguageVersion = languageVersion });

    [Fact]
    public void Hello_ReportsOurVersions()
    {
        var (responses, exit) = Converse(Envelope.Encode("Hello", ValidHello()), Envelope.Encode("Shutdown", new Shutdown()));
        Assert.Equal(0, exit);
        Assert.Equal(["Hello", "Ok"], responses.Select(r => r.Variant));
        var hello = Payload<Hello>(responses[0].Frame);
        Assert.Equal(CsharpGrammar.Hash, hello.GrammarHash);
        Assert.Equal(ProtocolInfo.Version, hello.ProtocolVersion);
    }

    [Theory]
    [InlineData("protocol")]
    [InlineData("grammar")]
    [InlineData("flow")]
    [InlineData("wire")]
    public void Hello_MismatchIsRefusedAndTheWorkerExits(string field)
    {
        var hello = ValidHello();
        switch (field)
        {
            case "protocol": hello.ProtocolVersion++; break;
            case "grammar": hello.GrammarHash = "stale"; break;
            case "flow": hello.FlowGraphFormat++; break;
            case "wire": hello.WireFormat++; break;
        }

        var (responses, exit) = Converse(Envelope.Encode("Hello", hello), ParseRequest("class A {}"));
        Assert.NotEqual(0, exit);
        var only = Assert.Single(responses);
        Assert.Equal("Error", only.Variant);
        Assert.Equal("version_mismatch", Payload<ProtocolError>(only.Frame).Code);
    }

    [Fact]
    public void RequestsBeforeHelloAreRejected()
    {
        var (responses, _) = Converse(ParseRequest("class A {}"));
        Assert.Equal("handshake_required", Payload<ProtocolError>(responses[0].Frame).Code);
    }

    [Theory]
    [InlineData("OpenWorkspace")]
    [InlineData("UpdateText")]
    [InlineData("Query")]
    [InlineData("FlowGraphs")]
    public void LaterMilestoneMessagesAreNotSupported(string variant)
    {
        byte[] request = variant switch
        {
            "OpenWorkspace" => Envelope.Encode(variant, new OpenWorkspace()),
            "UpdateText" => Envelope.Encode(variant, new UpdateText()),
            "Query" => Envelope.Encode(variant, new Query()),
            _ => Envelope.Encode(variant, new FlowGraphs()),
        };
        var (responses, _) = Converse(Envelope.Encode("Hello", ValidHello()), request);
        Assert.Equal("unsupported", Payload<ProtocolError>(responses[1].Frame).Code);
    }

    [Fact]
    public void UnknownMessageAndBadLanguageVersionAreBadRequests()
    {
        var (responses, _) = Converse(
            Envelope.Encode("Hello", ValidHello()),
            Envelope.Encode("Nope", new Shutdown()),
            ParseRequest("class A {}", "not-a-version"));
        Assert.Equal("bad_request", Payload<ProtocolError>(responses[1].Frame).Code);
        Assert.Equal("bad_request", Payload<ProtocolError>(responses[2].Frame).Code);
    }

    private static (Parsed Parsed, FlatTree Tree) ParseThroughServer(string text)
    {
        var (responses, _) = Converse(Envelope.Encode("Hello", ValidHello()), ParseRequest(text));
        Assert.Equal("Parsed", responses[1].Variant);
        var parsed = Payload<Parsed>(responses[1].Frame);
        return (parsed, WireEncoder.Decode(parsed.Tree));
    }

    /// <summary>Reconstructs token texts by walking the flat arrays; returns (kindName, text) for each token.</summary>
    private static List<(string Name, string Text)> Tokens(FlatTree tree, string text)
    {
        var names = CsharpGrammar.Kinds.ToDictionary(k => k.Id, k => k.Name);
        var bytes = Encoding.UTF8.GetBytes(text);
        var result = new List<(string, string)>();
        var offset = 0;
        for (var i = 0; i < tree.Count; i++)
        {
            if (GrammarKinds.IsToken(tree.Kinds[i]))
            {
                var width = (int)tree.Values[i];
                result.Add((names[tree.Kinds[i]], Encoding.UTF8.GetString(bytes, offset, width)));
                offset += width;
            }
        }

        Assert.Equal(bytes.Length, offset);
        return result;
    }

    [Fact]
    public void Parse_ProducesALosslessTreeWithSharedTriviaAttachment()
    {
        const string text = "int x; // c\n  /* b */ int y;\n";
        var (parsed, tree) = ParseThroughServer($"class A {{ {text} }}");
        Assert.Equal("a.cs", parsed.FileId);
        Assert.Equal(7ul, parsed.Version);
        var tokens = Tokens(tree, $"class A {{ {text} }}");
        var names = tokens.Select(t => t.Name).ToList();
        var semi = names.IndexOf("SemicolonToken");
        // The first statement's semicolon owns its trailing whitespace, comment and newline.
        Assert.Equal(["SemicolonToken", "WhitespaceTrivia", "SingleLineCommentTrivia", "EndOfLineTrivia"], names.Skip(semi).Take(4));
        // Leading trivia of the next token: indentation and the block comment.
        Assert.Equal(["WhitespaceTrivia", "MultiLineCommentTrivia", "WhitespaceTrivia", "IntKeyword"], names.Skip(semi + 4).Take(4));
        Assert.Equal("EndOfFileToken", names[^1]);
    }

    [Fact]
    public void Parse_HandlesMultiByteTextAndReportsErrorsInBytes()
    {
        const string text = "class A { string s = \"héllo🙂\"; int x = ; }";
        var (parsed, tree) = ParseThroughServer(text);
        Assert.Equal(text, string.Concat(Tokens(tree, text).Select(t => t.Text)));
        var error = Assert.Single(parsed.Errors);
        Assert.Equal("CS1525", error.Code);
        Assert.Equal((uint)Encoding.UTF8.GetByteCount(text[..text.IndexOf("; }", StringComparison.Ordinal)]), error.Start);
    }

    [Fact]
    public void Parse_KeepsEveryByteOfBrokenAndEmptyInput()
    {
        foreach (var text in new[] { "", "   \n", "class { ", "#if X\nint a;\n#endif\n#region r\n#endregion", "﻿class A {}", "x = $\"\"\"a{b}\"\"\";" })
        {
            var (_, tree) = ParseThroughServer(text);
            Assert.Equal(text, string.Concat(Tokens(tree, text).Select(t => t.Text)));
        }
    }
}
