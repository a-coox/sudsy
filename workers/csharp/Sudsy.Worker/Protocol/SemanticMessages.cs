using MessagePack;

namespace Sudsy.Worker.Protocol;

// Answers to Query and FlowGraphs. Mirrored by crates/provider-api/src/protocol.rs and, for the
// flow graph, crates/core-flow/src/graph.rs. Spans are UTF-8 byte offsets into the file text.

[MessagePackObject]
public sealed class Span
{
    [Key(0)] public uint Start { get; set; }
    [Key(1)] public uint Length { get; set; }
}

[MessagePackObject]
public sealed class Declaration
{
    /// <summary>Documentation comment id, e.g. M:Shop.Orders.OrderService.GetRecentAsync(System.Int32).</summary>
    [Key(0)] public string Symbol { get; set; } = "";
    /// <summary>0 type, 1 method, 2 field, 3 property, 4 event.</summary>
    [Key(1)] public byte Kind { get; set; }
    [Key(2)] public string Name { get; set; } = "";
    /// <summary>The name's span.</summary>
    [Key(3)] public uint Start { get; set; }
    [Key(4)] public uint Length { get; set; }
    /// <summary>0 private, 1 internal, 2 protected (any kind), 3 public.</summary>
    [Key(5)] public byte Accessibility { get; set; }
    /// <summary>Async 1, override 2, implements an interface member 4, explicit implementation 8,
    /// has attributes 16, extern or partial 32, returns Task, ValueTask or a task-like type 64,
    /// static 128.</summary>
    [Key(6)] public byte Flags { get; set; }
}

[MessagePackObject]
public sealed class Reference
{
    [Key(0)] public string Symbol { get; set; } = "";
    [Key(1)] public uint Start { get; set; }
    [Key(2)] public uint Length { get; set; }
}

[MessagePackObject]
public sealed class CompilerDiagnostic
{
    [Key(0)] public string Id { get; set; } = "";
    [Key(1)] public uint Start { get; set; }
    [Key(2)] public uint Length { get; set; }
    [Key(3)] public string Message { get; set; } = "";
}

[MessagePackObject]
public sealed class Finding
{
    /// <summary>The check: base_call, default_value_constructor, unqualified_using, value_tuple,
    /// unnamed_tuple_element, debug_assert_message, debug_fail_message, inheritdoc.</summary>
    [Key(0)] public string Kind { get; set; } = "";
    [Key(1)] public uint Start { get; set; }
    [Key(2)] public uint Length { get; set; }
    /// <summary>The replacement for the span, or what the message needs; empty for none.</summary>
    [Key(3)] public string Text { get; set; } = "";
    /// <summary>The range `Text` replaces (a part of the span or the span itself); 0, 0 for the span.</summary>
    [Key(4)] public uint FixStart { get; set; }
    [Key(5)] public uint FixLength { get; set; }
}

[MessagePackObject]
public sealed class Answers
{
    [Key(0)] public string FileId { get; set; } = "";
    [Key(1)] public ulong Version { get; set; }
    [Key(2)] public Declaration[] Declarations { get; set; } = [];
    [Key(3)] public Reference[] References { get; set; } = [];
    [Key(4)] public Span[] UnusedImports { get; set; } = [];
    [Key(5)] public Span[] MaybeNull { get; set; } = [];
    [Key(6)] public CompilerDiagnostic[] Diagnostics { get; set; } = [];
    /// <summary>Compiler errors in the file; answers that need a complete binding (unused imports)
    /// are left empty when there are any.</summary>
    [Key(7)] public uint SemanticErrors { get; set; }
    [Key(8)] public Finding[] Findings { get; set; } = [];
}

[MessagePackObject]
public sealed class FlowOp
{
    [Key(0)] public byte Kind { get; set; }
    [Key(1)] public int Local { get; set; } = -1;
    [Key(2)] public uint Start { get; set; }
    [Key(3)] public uint Len { get; set; }
    [Key(4)] public uint StmtStart { get; set; }
    [Key(5)] public uint StmtLen { get; set; }
    [Key(6)] public string Callee { get; set; } = "";
    [Key(7)] public byte Flags { get; set; }
}

[MessagePackObject]
public sealed class FlowEdge
{
    [Key(0)] public uint To { get; set; }
    [Key(1)] public byte Kind { get; set; }
}

[MessagePackObject]
public sealed class FlowBlock
{
    [Key(0)] public FlowOp[] Ops { get; set; } = [];
    [Key(1)] public FlowEdge[] Succ { get; set; } = [];
    [Key(2)] public bool? BranchConst { get; set; }
}

[MessagePackObject]
public sealed class FlowLocal
{
    [Key(0)] public string Name { get; set; } = "";
    [Key(1)] public uint Start { get; set; }
    [Key(2)] public uint Len { get; set; }
    [Key(3)] public byte Kind { get; set; }
    [Key(4)] public byte Flags { get; set; }
}

[MessagePackObject]
public sealed class BodyGraph
{
    [Key(0)] public byte Kind { get; set; }
    [Key(1)] public uint Start { get; set; }
    [Key(2)] public uint Len { get; set; }
    [Key(3)] public string Symbol { get; set; } = "";
    [Key(4)] public FlowLocal[] Locals { get; set; } = [];
    [Key(5)] public FlowBlock[] Blocks { get; set; } = [];
}

[MessagePackObject]
public sealed class Graphs
{
    [Key(0)] public string FileId { get; set; } = "";
    [Key(1)] public ulong Version { get; set; }
    [Key(2)] public BodyGraph[] Bodies { get; set; } = [];
}
