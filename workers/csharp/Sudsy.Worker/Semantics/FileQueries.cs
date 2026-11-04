using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Sudsy.Worker.Protocol;

namespace Sudsy.Worker.Semantics;

/// <summary>Binding, type and value answers about one file, in one batch.</summary>
internal static class FileQueries
{
    public static Answers Answer(Workspace workspace, Document document, SemanticModel model, Query query)
    {
        var answers = new Answers { FileId = query.FileId, Version = query.Version };
        var diagnostics = workspace.Diagnostics(document, model);
        answers.SemanticErrors = (uint)diagnostics.Count(d => d.Severity == DiagnosticSeverity.Error);
        foreach (var item in query.Items)
        {
            switch (item)
            {
                case "declarations":
                    answers.Declarations = Declarations(document, model);
                    break;
                case "references":
                    answers.References = References(document, model);
                    break;
                case "unused_imports":
                    answers.UnusedImports = answers.SemanticErrors == 0 ? UnusedImports(document, diagnostics) : [];
                    break;
                case "symbol_findings":
                    answers.Findings = SymbolFindings.Collect(document, model);
                    break;
                case "maybe_null":
                    answers.MaybeNull = MaybeNull(document, model);
                    break;
                case "diagnostics":
                    answers.Diagnostics = diagnostics
                        .Where(d => query.DiagnosticIds.Contains(d.Id))
                        .Select(d => new CompilerDiagnostic
                        {
                            Id = d.Id,
                            Start = document.Offsets.At(d.Location.SourceSpan.Start),
                            Length = document.Offsets.Width(d.Location.SourceSpan),
                            Message = d.GetMessage(),
                        })
                        .ToArray();
                    break;
                default:
                    throw new ArgumentException($"unknown query item {item}");
            }
        }

        return answers;
    }

    private static Declaration[] Declarations(Document document, SemanticModel model)
    {
        var result = new List<Declaration>();
        foreach (var node in document.Tree.GetRoot().DescendantNodes(n => n is not (BlockSyntax or ArrowExpressionClauseSyntax)))
        {
            switch (node)
            {
                case BaseTypeDeclarationSyntax t:
                    Add(model.GetDeclaredSymbol(t), 0, t.Identifier);
                    break;
                case DelegateDeclarationSyntax d:
                    Add(model.GetDeclaredSymbol(d), 0, d.Identifier);
                    break;
                case MethodDeclarationSyntax m:
                    Add(model.GetDeclaredSymbol(m), 1, m.Identifier);
                    break;
                case PropertyDeclarationSyntax p:
                    Add(model.GetDeclaredSymbol(p), 3, p.Identifier);
                    break;
                case VariableDeclaratorSyntax { Parent.Parent: FieldDeclarationSyntax }:
                    Add(model.GetDeclaredSymbol(node), 2, ((VariableDeclaratorSyntax)node).Identifier);
                    break;
                case VariableDeclaratorSyntax { Parent.Parent: EventFieldDeclarationSyntax }:
                    Add(model.GetDeclaredSymbol(node), 4, ((VariableDeclaratorSyntax)node).Identifier);
                    break;
                case EventDeclarationSyntax e:
                    Add(model.GetDeclaredSymbol(e), 4, e.Identifier);
                    break;
            }
        }

        return result.ToArray();

        void Add(ISymbol? symbol, byte kind, SyntaxToken name)
        {
            var id = symbol is null ? null : DocumentationCommentId.CreateDeclarationId(symbol);
            if (symbol is null || id is null)
            {
                return;
            }

            result.Add(new Declaration
            {
                Symbol = id,
                Kind = kind,
                Name = symbol.Name,
                Start = document.Offsets.At(name.Span.Start),
                Length = document.Offsets.Width(name.Span),
                Accessibility = symbol.DeclaredAccessibility switch
                {
                    Accessibility.Internal => 1,
                    Accessibility.Protected or Accessibility.ProtectedAndInternal or Accessibility.ProtectedOrInternal => 2,
                    Accessibility.Public => 3,
                    _ => 0,
                },
                Flags = FlagsOf(symbol),
            });
        }
    }

    private static byte FlagsOf(ISymbol symbol)
    {
        byte flags = 0;
        if (symbol is IMethodSymbol { IsAsync: true })
        {
            flags |= 1;
        }

        if (symbol.IsOverride)
        {
            flags |= 2;
        }

        if (Implements(symbol))
        {
            flags |= 4;
        }

        if (symbol is IMethodSymbol { ExplicitInterfaceImplementations.Length: > 0 }
            or IPropertySymbol { ExplicitInterfaceImplementations.Length: > 0 })
        {
            flags |= 8;
        }

        if (symbol.GetAttributes().Length > 0)
        {
            flags |= 16;
        }

        if (symbol is IMethodSymbol { IsExtern: true } or IMethodSymbol { PartialDefinitionPart: not null } or IMethodSymbol { PartialImplementationPart: not null })
        {
            flags |= 32;
        }

        if (symbol is IMethodSymbol { ReturnType: INamedTypeSymbol t } && IsTask(t))
        {
            flags |= 64;
        }

        if (symbol.IsStatic)
        {
            flags |= 128;
        }

        return flags;
    }

    private static bool IsTask(INamedTypeSymbol type) =>
        type.OriginalDefinition.ToDisplayString() is "System.Threading.Tasks.Task" or "System.Threading.Tasks.Task<TResult>"
            or "System.Threading.Tasks.ValueTask" or "System.Threading.Tasks.ValueTask<TResult>";

    internal static bool Implements(ISymbol symbol)
    {
        if (symbol is not (IMethodSymbol or IPropertySymbol or IEventSymbol) || symbol.ContainingType is not { } type)
        {
            return false;
        }

        foreach (var iface in type.AllInterfaces)
        {
            foreach (var member in iface.GetMembers(symbol.Name))
            {
                if (SymbolEqualityComparer.Default.Equals(type.FindImplementationForInterfaceMember(member), symbol))
                {
                    return true;
                }
            }
        }

        return false;
    }

    private static Reference[] References(Document document, SemanticModel model)
    {
        var result = new List<Reference>();
        foreach (var node in document.Tree.GetRoot().DescendantNodes().OfType<SimpleNameSyntax>())
        {
            var info = model.GetSymbolInfo(node);
            var symbol = info.Symbol ?? info.CandidateSymbols.FirstOrDefault();
            if (symbol is not (IFieldSymbol or IMethodSymbol or IPropertySymbol or IEventSymbol or INamedTypeSymbol)
                || !symbol.Locations.Any(l => l.IsInSource))
            {
                continue;
            }

            var id = DocumentationCommentId.CreateDeclarationId(symbol.OriginalDefinition);
            if (id is not null)
            {
                result.Add(new Reference
                {
                    Symbol = id,
                    Start = document.Offsets.At(node.Span.Start),
                    Length = document.Offsets.Width(node.Span),
                });
            }
        }

        return result.ToArray();
    }

    /// <summary>Unnecessary using directives, unless inactive code could use them (conditional
    /// compilation) or the file is not fully bound.</summary>
    private static Span[] UnusedImports(Document document, IReadOnlyList<Diagnostic> diagnostics)
    {
        var root = document.Tree.GetRoot();
        if (root.ContainsDirectives && root.DescendantNodes(descendIntoTrivia: true).OfType<IfDirectiveTriviaSyntax>().Any())
        {
            return [];
        }

        return diagnostics
            .Where(d => d.Id == "CS8019")
            .Select(d => root.FindNode(d.Location.SourceSpan))
            .OfType<UsingDirectiveSyntax>()
            .Where(u => u.GlobalKeyword.IsKind(SyntaxKind.None))
            .Select(u => new Span { Start = document.Offsets.At(u.Span.Start), Length = document.Offsets.Width(u.Span) })
            .ToArray();
    }

    private static Span[] MaybeNull(Document document, SemanticModel model)
    {
        var result = new List<Span>();
        foreach (var node in document.Tree.GetRoot().DescendantNodes())
        {
            var receiver = node switch
            {
                MemberAccessExpressionSyntax { RawKind: (int)SyntaxKind.SimpleMemberAccessExpression } m => m.Expression,
                ElementAccessExpressionSyntax e => e.Expression,
                _ => null,
            };
            if (receiver is not null
                && model.GetTypeInfo(receiver).Nullability.FlowState == NullableFlowState.MaybeNull)
            {
                result.Add(new Span { Start = document.Offsets.At(receiver.Span.Start), Length = document.Offsets.Width(receiver.Span) });
            }
        }

        return result.ToArray();
    }
}
