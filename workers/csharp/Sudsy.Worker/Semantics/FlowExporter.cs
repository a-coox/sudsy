using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.FlowAnalysis;
using Microsoft.CodeAnalysis.Operations;
using Sudsy.Worker.Protocol;

namespace Sudsy.Worker.Semantics;

/// <summary>
/// Converts Roslyn's operation-based control-flow graphs into the language-neutral graphs of
/// crates/core-flow: simple operations carrying local indices, callee symbol ids and constants.
/// The analyses run in the core.
/// </summary>
internal static class FlowExporter
{
    // Codes shared with crates/core-flow/src/graph.rs.
    private const byte Read = 0, Write = 1, ReadWrite = 2, Declare = 3, Call = 4, Other = 5;
    private const byte Constant = 1, NoReturn = 2, Pure = 4;
    private const byte Always = 0, WhenTrue = 1, WhenFalse = 2, Handler = 3;
    private const byte Declaration = 0, CatchVariable = 1, OtherLocal = 3;
    private const byte Captured = 1, AddressTaken = 2;
    private const byte MethodBody = 0, Lambda = 1, LocalFunction = 2;

    public static BodyGraph[] Export(Document document, SemanticModel model)
    {
        var graphs = new List<BodyGraph>();
        foreach (var node in document.Tree.GetRoot().DescendantNodes())
        {
            if (node is BaseMethodDeclarationSyntax or AccessorDeclarationSyntax && HasBody(node))
            {
                try
                {
                    if (model.GetOperation(node) is { } op && Create(op) is { } cfg)
                    {
                        new Builder(model, document, graphs).Export(cfg, MethodBody, node, DocId(model.GetDeclaredSymbol(node)));
                    }
                }
                catch (Exception e) when (e is InvalidOperationException or ArgumentException)
                {
                    // A body Roslyn cannot build a graph for has none; rules skip it.
                }
            }
        }

        return graphs.ToArray();
    }

    private static bool HasBody(SyntaxNode node) => node switch
    {
        BaseMethodDeclarationSyntax m => m.Body != null || m.ExpressionBody != null,
        AccessorDeclarationSyntax a => a.Body != null || a.ExpressionBody != null,
        _ => false,
    };

    private static ControlFlowGraph? Create(IOperation op) => op switch
    {
        IMethodBodyOperation m => ControlFlowGraph.Create(m),
        IConstructorBodyOperation c => ControlFlowGraph.Create(c),
        _ => null,
    };

    private static string DocId(ISymbol? symbol) => symbol is null ? "" : DocumentationCommentId.CreateDeclarationId(symbol) ?? "";

    private sealed class Builder(SemanticModel model, Document document, List<BodyGraph> output)
    {
        public Document Document => document;

        /// <summary>Exports the graph and the graphs nested in it; returns the outer locals it uses.</summary>
        public HashSet<ILocalSymbol> Export(ControlFlowGraph cfg, byte kind, SyntaxNode owner, string symbol)
        {
            var locals = new List<ILocalSymbol>();
            Collect(cfg.Root, locals);
            var index = new Dictionary<ILocalSymbol, int>(SymbolEqualityComparer.Default);
            for (var i = 0; i < locals.Count; i++)
            {
                index[locals[i]] = i;
            }

            var flags = new byte[locals.Count];
            var outer = new HashSet<ILocalSymbol>(SymbolEqualityComparer.Default);
            var nested = new List<(ControlFlowGraph Graph, byte Kind, SyntaxNode Owner, string Symbol)>();

            var blocks = new FlowBlock[cfg.Blocks.Length];
            foreach (var block in cfg.Blocks)
            {
                var ops = new List<FlowOp>();
                var walker = new Walker(this, index, flags, outer, ops, cfg, nested);
                // Every root operation leaves a mark, so a statement that touches no local and calls
                // nothing still shows which block it is in.
                foreach (var op in block.Operations.Concat(block.BranchValue is { } branch ? [branch] : []))
                {
                    walker.Mark(op);
                    walker.Walk(op);
                }

                blocks[block.Ordinal] = new FlowBlock
                {
                    Ops = ops.ToArray(),
                    Succ = Successors(block),
                    BranchConst = block.BranchValue?.ConstantValue is { HasValue: true, Value: bool b } ? b : null,
                };
            }

            AddHandlerEdges(cfg.Root, blocks);

            foreach (var fn in cfg.LocalFunctions)
            {
                var syntax = fn.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax();
                if (syntax is not null)
                {
                    nested.Add((cfg.GetLocalFunctionControlFlowGraph(fn), LocalFunction, syntax, DocId(fn)));
                }
            }

            // Locals used by nested bodies cannot be placed in time.
            foreach (var (graph, nestedKind, syntax, id) in nested)
            {
                foreach (var l in new Builder(model, document, output).Export(graph, nestedKind, syntax, id))
                {
                    if (index.TryGetValue(l, out var i))
                    {
                        flags[i] |= Captured;
                    }
                    else
                    {
                        outer.Add(l);
                    }
                }
            }

            output.Add(new BodyGraph
            {
                Kind = kind,
                Start = document.Offsets.At(owner.Span.Start),
                Len = document.Offsets.Width(owner.Span),
                Symbol = symbol,
                Locals = locals.Select((l, i) => Describe(l, flags[i])).ToArray(),
                Blocks = blocks,
            });
            return outer;
        }

        private FlowLocal Describe(ILocalSymbol local, byte flags)
        {
            var span = local.Locations.FirstOrDefault(l => l.IsInSource)?.SourceSpan ?? default;
            var syntax = local.DeclaringSyntaxReferences.FirstOrDefault()?.GetSyntax();
            var kind = syntax switch
            {
                CatchDeclarationSyntax => CatchVariable,
                VariableDeclaratorSyntax { Parent: VariableDeclarationSyntax { Parent: LocalDeclarationStatementSyntax { UsingKeyword.RawKind: 0, AwaitKeyword.RawKind: 0 } } }
                    when !local.IsConst => Declaration,
                VariableDeclaratorSyntax { Parent: VariableDeclarationSyntax { Parent: ForStatementSyntax } } => Declaration,
                _ => OtherLocal,
            };
            return new FlowLocal
            {
                Name = local.Name ?? "",
                Start = document.Offsets.At(span.Start),
                Len = document.Offsets.Width(span),
                Kind = (byte)kind,
                Flags = flags,
            };
        }

        private static void Collect(ControlFlowRegion region, List<ILocalSymbol> into)
        {
            into.AddRange(region.Locals);
            foreach (var nested in region.NestedRegions)
            {
                Collect(nested, into);
            }
        }

        private static FlowEdge[] Successors(BasicBlock block)
        {
            var edges = new List<FlowEdge>();
            var conditional = block.ConditionalSuccessor?.Destination;
            var whenTrue = block.ConditionKind == ControlFlowConditionKind.WhenTrue;
            if (conditional is not null)
            {
                edges.Add(new FlowEdge { To = (uint)conditional.Ordinal, Kind = whenTrue ? WhenTrue : WhenFalse });
            }

            if (block.FallThroughSuccessor?.Destination is { } next)
            {
                var kind = conditional is null ? Always : whenTrue ? WhenFalse : WhenTrue;
                edges.Add(new FlowEdge { To = (uint)next.Ordinal, Kind = kind });
            }

            return edges.ToArray();
        }

        /// <summary>Every block of a try may throw into the start of each of its handlers.</summary>
        private static void AddHandlerEdges(ControlFlowRegion region, FlowBlock[] blocks)
        {
            if (region.Kind is ControlFlowRegionKind.TryAndCatch or ControlFlowRegionKind.TryAndFinally
                && region.NestedRegions.Length > 1)
            {
                var tryRegion = region.NestedRegions[0];
                foreach (var handler in region.NestedRegions.Skip(1))
                {
                    for (var b = tryRegion.FirstBlockOrdinal; b <= tryRegion.LastBlockOrdinal; b++)
                    {
                        blocks[b].Succ = [.. blocks[b].Succ, new FlowEdge { To = (uint)handler.FirstBlockOrdinal, Kind = Handler }];
                    }
                }
            }

            foreach (var nested in region.NestedRegions)
            {
                AddHandlerEdges(nested, blocks);
            }
        }

        /// <summary>Walks one block's operation trees in evaluation order.</summary>
        private sealed class Walker(
            Builder owner,
            Dictionary<ILocalSymbol, int> index,
            byte[] flags,
            HashSet<ILocalSymbol> outer,
            List<FlowOp> ops,
            ControlFlowGraph cfg,
            List<(ControlFlowGraph Graph, byte Kind, SyntaxNode Owner, string Symbol)> nested)
        {
            public void Mark(IOperation op) => ops.Add(MakeOp(Other, -1, 0, "", op.Syntax));

            public void Walk(IOperation op)
            {
                switch (op)
                {
                    case ISimpleAssignmentOperation a:
                        Walk(a.Value);
                        if (a.IsRef)
                        {
                            Mark(a.Value, AddressTaken);
                        }

                        if (a.Syntax is CatchDeclarationSyntax or ForEachStatementSyntax or ForEachVariableStatementSyntax)
                        {
                            Assign(a.Target, Declare, false, a.Syntax);
                        }
                        else
                        {
                            Assign(a.Target, Write, a.Value.ConstantValue.HasValue, a.Syntax);
                        }

                        return;
                    case ICompoundAssignmentOperation c:
                        Walk(c.Value);
                        Assign(c.Target, ReadWrite, false, c.Syntax);
                        return;
                    case IIncrementOrDecrementOperation i:
                        Assign(i.Target, ReadWrite, false, i.Syntax);
                        return;
                    case IDeconstructionAssignmentOperation d:
                        Walk(d.Value);
                        Assign(d.Target, Write, false, d.Syntax);
                        return;
                    case IAddressOfOperation addr:
                        Mark(addr.Reference, AddressTaken);
                        break;
                    case IArgumentOperation { Parameter.RefKind: RefKind.Out } arg:
                        Assign(arg.Value, Write, false, arg.Syntax);
                        return;
                    case IArgumentOperation { Parameter.RefKind: RefKind.Ref } arg:
                        Assign(arg.Value, ReadWrite, false, arg.Syntax);
                        return;
                    case ILocalReferenceOperation r:
                        Emit(Read, r.Local, false, r.Syntax);
                        return;
                    case IFlowAnonymousFunctionOperation f:
                        nested.Add((cfg.GetAnonymousFunctionControlFlowGraph(f), Lambda, f.Syntax, ""));
                        return;
                }

                foreach (var child in op.ChildOperations)
                {
                    Walk(child);
                }

                if (op is IInvocationOperation call)
                {
                    EmitCall(call);
                }
            }

            private void Mark(IOperation target, byte flag)
            {
                if (target is ILocalReferenceOperation r && index.TryGetValue(r.Local, out var i))
                {
                    flags[i] |= flag;
                }
            }

            /// <summary>A store (or declaration) into whatever the target is; other targets (fields,
            /// elements) only evaluate their receivers.</summary>
            private void Assign(IOperation target, byte kind, bool constant, SyntaxNode syntax)
            {
                switch (target)
                {
                    case ILocalReferenceOperation r:
                        Emit(kind, r.Local, constant, syntax);
                        break;
                    case IDeclarationExpressionOperation d:
                        Assign(d.Expression, kind, constant, syntax);
                        break;
                    case ITupleOperation t:
                        foreach (var element in t.Elements)
                        {
                            Assign(element, kind, false, syntax);
                        }

                        break;
                    default:
                        foreach (var child in target.ChildOperations)
                        {
                            Walk(child);
                        }

                        break;
                }
            }

            private void Emit(byte kind, ILocalSymbol local, bool constant, SyntaxNode syntax)
            {
                if (!index.TryGetValue(local, out var i))
                {
                    outer.Add(local);
                    return;
                }

                ops.Add(MakeOp(kind, i, constant ? Constant : (byte)0, "", syntax));
            }

            private void EmitCall(IInvocationOperation call)
            {
                byte f = 0;
                foreach (var attribute in call.TargetMethod.GetAttributes())
                {
                    switch (attribute.AttributeClass?.Name)
                    {
                        case "DoesNotReturnAttribute":
                            f |= NoReturn;
                            break;
                        case "PureAttribute":
                            f |= Pure;
                            break;
                    }
                }

                ops.Add(MakeOp(Call, -1, f, DocId(call.TargetMethod.OriginalDefinition), call.Syntax));
            }

            private FlowOp MakeOp(byte kind, int local, byte opFlags, string callee, SyntaxNode syntax)
            {
                var statement = syntax.AncestorsAndSelf().OfType<StatementSyntax>().FirstOrDefault(s => s is not BlockSyntax);
                var offsets = owner.Document.Offsets;
                return new FlowOp
                {
                    Kind = kind,
                    Local = local,
                    Start = offsets.At(syntax.Span.Start),
                    Len = offsets.Width(syntax.Span),
                    StmtStart = statement is null ? 0 : offsets.At(statement.Span.Start),
                    StmtLen = statement is null ? 0 : offsets.Width(statement.Span),
                    Callee = callee,
                    Flags = opFlags,
                };
            }
        }
    }
}
