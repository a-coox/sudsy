using System.Text;
using MessagePack;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Sudsy.Worker.Generated;
using Sudsy.Worker.Mapping;
using Sudsy.Worker.Protocol;
using Sudsy.Worker.Semantics;

namespace Sudsy.Worker;

/// <summary>Serves the worker protocol on a pair of streams, one request at a time.</summary>
public sealed class WorkerServer(Stream input, Stream output)
{
    private static readonly string[] Capabilities = ["binding", "types", "values", "flow", "project"];

    private readonly Workspace _workspace = new();

    private bool _greeted;

    /// <summary>Runs until Shutdown, end of input or a refused handshake. Returns the process exit code.</summary>
    public int Run()
    {
        while (true)
        {
            var frame = Envelope.ReadFrame(input);
            if (frame is null)
            {
                return 0;
            }

            var (response, exitCode) = Handle(frame);
            Envelope.WriteFrame(output, response);
            if (exitCode is { } code)
            {
                return code;
            }
        }
    }

    private (byte[] Response, int? ExitCode) Handle(byte[] frame)
    {
        try
        {
            var reader = new MessagePackReader(frame);
            var variant = Envelope.ReadVariant(ref reader);
            if (variant == "Hello")
            {
                return Greet(Envelope.Payload<Hello>(ref reader));
            }

            if (!_greeted)
            {
                return (Error("handshake_required", "send Hello first"), null);
            }

            return variant switch
            {
                "Parse" => (Parse(Envelope.Payload<ParseRequest>(ref reader)), null),
                "Shutdown" => (Envelope.Encode("Ok", new Ok()), 0),
                "OpenWorkspace" => (OpenWorkspace(Envelope.Payload<OpenWorkspace>(ref reader)), null),
                "UpdateText" => (UpdateText(Envelope.Payload<UpdateText>(ref reader)), null),
                "Query" => (Query(Envelope.Payload<Query>(ref reader)), null),
                "FlowGraphs" => (FlowGraphs(Envelope.Payload<FlowGraphs>(ref reader)), null),
                _ => (Error("bad_request", $"unknown message {variant}"), null),
            };
        }
        catch (Exception e) when (e is MessagePackSerializationException or InvalidDataException)
        {
            return (Error("bad_request", e.Message), null);
        }
        catch (Exception e)
        {
            return (Error("internal", e.ToString()), null);
        }
    }

    private (byte[] Response, int? ExitCode) Greet(Hello theirs)
    {
        var ours = new Hello
        {
            ProtocolVersion = ProtocolInfo.Version,
            GrammarHash = CsharpGrammar.Hash,
            FlowGraphFormat = ProtocolInfo.FlowGraphFormatVersion,
            WireFormat = ProtocolInfo.WireFormatVersion,
            Capabilities = Capabilities,
        };
        var mismatch = new List<string>();
        if (theirs.ProtocolVersion != ours.ProtocolVersion)
        {
            mismatch.Add($"protocol {theirs.ProtocolVersion} vs {ours.ProtocolVersion}");
        }

        if (theirs.GrammarHash != ours.GrammarHash)
        {
            mismatch.Add($"grammar hash {theirs.GrammarHash} vs {ours.GrammarHash}");
        }

        if (theirs.FlowGraphFormat != ours.FlowGraphFormat)
        {
            mismatch.Add($"flow-graph format {theirs.FlowGraphFormat} vs {ours.FlowGraphFormat}");
        }

        if (theirs.WireFormat != ours.WireFormat)
        {
            mismatch.Add($"wire format {theirs.WireFormat} vs {ours.WireFormat}");
        }

        if (mismatch.Count > 0)
        {
            return (Error("version_mismatch", string.Join("; ", mismatch)), 3);
        }

        _greeted = true;
        return (Envelope.Encode("Hello", ours), null);
    }

    private static byte[] Parse(ParseRequest request)
    {
        var languageVersion = LanguageVersion.Latest;
        if (request.LanguageVersion is { } v && !LanguageVersionFacts.TryParse(v, out languageVersion))
        {
            return Error("bad_request", $"unknown language version '{v}'");
        }

        var options = new CSharpParseOptions(languageVersion, preprocessorSymbols: request.PreprocessorSymbols);
        var tree = CSharpSyntaxTree.ParseText(request.Text, options);
        var root = tree.GetRoot();
        var flat = TreeMapper.Map(root, request.Text);
        var errors = tree.GetDiagnostics(root)
            .Where(d => d.Severity == DiagnosticSeverity.Error)
            .Select(d => new ParseError
            {
                Start = (uint)Encoding.UTF8.GetByteCount(request.Text.AsSpan(0, d.Location.SourceSpan.Start)),
                Length = (uint)Encoding.UTF8.GetByteCount(request.Text.AsSpan(d.Location.SourceSpan.Start, d.Location.SourceSpan.Length)),
                Code = d.Id,
                Message = d.GetMessage(),
            })
            .ToArray();
        return Envelope.Encode("Parsed", new Parsed
        {
            FileId = request.FileId,
            Version = request.Version,
            Tree = WireEncoder.Encode(flat),
            Errors = errors,
        });
    }

    private byte[] OpenWorkspace(OpenWorkspace request)
    {
        _workspace.Open(request.Paths);
        return Envelope.Encode("Ok", new Ok());
    }

    private byte[] UpdateText(UpdateText request)
    {
        _workspace.Update(request.FileId, request.Version, request.Text);
        return Envelope.Encode("Ok", new Ok());
    }

    private byte[] Query(Query request)
    {
        if (_workspace.Get(request.FileId) is not var (document, model))
        {
            return Error("bad_request", $"{request.FileId} is not in the workspace; send UpdateText first");
        }

        return Envelope.Encode("Answers", FileQueries.Answer(_workspace, document, model, request));
    }

    private byte[] FlowGraphs(FlowGraphs request)
    {
        if (_workspace.Get(request.FileId) is not var (document, model))
        {
            return Error("bad_request", $"{request.FileId} is not in the workspace; send UpdateText first");
        }

        return Envelope.Encode("Graphs", new Graphs
        {
            FileId = request.FileId,
            Version = request.Version,
            Bodies = FlowExporter.Export(document, model),
        });
    }

    private static byte[] Error(string code, string message) =>
        Envelope.Encode("Error", new ProtocolError { Code = code, Message = message });
}
