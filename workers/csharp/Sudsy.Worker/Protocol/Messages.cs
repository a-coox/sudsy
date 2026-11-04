using MessagePack;

namespace Sudsy.Worker.Protocol;

/// <summary>
/// Worker protocol v1 payloads. Each is a MessagePack array of its fields in Key order, which is
/// what Rust's rmp-serde writes for structs (crates/provider-api/src/protocol.rs mirrors this file).
/// </summary>
public static class ProtocolInfo
{
    public const uint Version = 3;
    public const uint FlowGraphFormatVersion = 1;
    public const uint WireFormatVersion = 1;
}

[MessagePackObject]
public sealed class Hello
{
    [Key(0)] public uint ProtocolVersion { get; set; }
    [Key(1)] public string GrammarHash { get; set; } = "";
    [Key(2)] public uint FlowGraphFormat { get; set; }
    [Key(3)] public uint WireFormat { get; set; }
    /// <summary>Any of: binding, types, values, flow, project.</summary>
    [Key(4)] public string[] Capabilities { get; set; } = [];
}

[MessagePackObject]
public sealed class OpenWorkspace
{
    [Key(0)] public string[] Paths { get; set; } = [];
    [Key(1)] public Dictionary<string, string> Properties { get; set; } = new();
}

[MessagePackObject]
public sealed class ParseRequest
{
    [Key(0)] public string FileId { get; set; } = "";
    [Key(1)] public ulong Version { get; set; }
    [Key(2)] public string Text { get; set; } = "";
    /// <summary>Language version such as "12" or "latest"; null means latest.</summary>
    [Key(3)] public string? LanguageVersion { get; set; }
    /// <summary>Conditional compilation symbols; empty for none.</summary>
    [Key(4)] public string[] PreprocessorSymbols { get; set; } = [];
}

[MessagePackObject]
public sealed class UpdateText
{
    [Key(0)] public string FileId { get; set; } = "";
    [Key(1)] public ulong Version { get; set; }
    [Key(2)] public string Text { get; set; } = "";
}

[MessagePackObject]
public sealed class Query
{
    [Key(0)] public string FileId { get; set; } = "";
    [Key(1)] public ulong Version { get; set; }
    /// <summary>What to answer: declarations, references, unused_imports, maybe_null, symbol_findings, diagnostics.</summary>
    [Key(2)] public string[] Items { get; set; } = [];
    /// <summary>For the diagnostics item: the compiler diagnostic ids to report (CS0162, ...).</summary>
    [Key(3)] public string[] DiagnosticIds { get; set; } = [];
}

[MessagePackObject]
public sealed class FlowGraphs
{
    [Key(0)] public string FileId { get; set; } = "";
    [Key(1)] public ulong Version { get; set; }
}

[MessagePackObject]
public sealed class Shutdown;

[MessagePackObject]
public sealed class WireTree
{
    [Key(0)] public uint Version { get; set; }
    [Key(1)] public byte[] Kinds { get; set; } = [];
    [Key(2)] public byte[] Values { get; set; } = [];
}

[MessagePackObject]
public sealed class ParseError
{
    /// <summary>Byte offset (UTF-8) into the text.</summary>
    [Key(0)] public uint Start { get; set; }
    [Key(1)] public uint Length { get; set; }
    [Key(2)] public string Code { get; set; } = "";
    [Key(3)] public string Message { get; set; } = "";
}

[MessagePackObject]
public sealed class Parsed
{
    [Key(0)] public string FileId { get; set; } = "";
    [Key(1)] public ulong Version { get; set; }
    [Key(2)] public WireTree Tree { get; set; } = new();
    [Key(3)] public ParseError[] Errors { get; set; } = [];
}

[MessagePackObject]
public sealed class Ok;

[MessagePackObject]
public sealed class ProtocolError
{
    /// <summary>version_mismatch, handshake_required, unsupported, bad_request, internal.</summary>
    [Key(0)] public string Code { get; set; } = "";
    [Key(1)] public string Message { get; set; } = "";
}
