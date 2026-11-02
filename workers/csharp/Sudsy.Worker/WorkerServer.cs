using System.Text;
using MessagePack;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Sudsy.Worker.Generated;
using Sudsy.Worker.Mapping;
using Sudsy.Worker.Protocol;

namespace Sudsy.Worker;

/// <summary>Serves protocol v1 on a pair of streams, one request at a time.</summary>
public sealed class WorkerServer(Stream input, Stream output)
{
    private static readonly string[] Capabilities = [];

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
                "OpenWorkspace" or "UpdateText" or "Query" or "FlowGraphs" =>
                    (Error("unsupported", $"{variant} is not supported by this worker yet"), null),
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

    private static byte[] Error(string code, string message) =>
        Envelope.Encode("Error", new ProtocolError { Code = code, Message = message });
}
