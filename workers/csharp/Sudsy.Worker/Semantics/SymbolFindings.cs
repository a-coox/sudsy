using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Operations;
using Microsoft.CodeAnalysis.Text;
using Sudsy.Worker.Protocol;

namespace Sudsy.Worker.Semantics;

/// <summary>Checks that need a symbol or a type, one walk over the file: base calls that could be
/// this calls, default value-type constructors, partly qualified usings in namespaces,
/// ValueTuple spelled out, tuple elements without names in signatures, Debug.Assert/Fail
/// without a message, and inheritdoc on members that inherit nothing.</summary>
internal sealed class SymbolFindings
{
    private static readonly SymbolDisplayFormat Qualified =
        SymbolDisplayFormat.FullyQualifiedFormat.WithGlobalNamespaceStyle(SymbolDisplayGlobalNamespaceStyle.Omitted);

    private readonly Document document;
    private readonly SemanticModel model;
    private readonly List<Finding> result = [];
    private readonly INamedTypeSymbol? expressionType;
    private readonly INamedTypeSymbol? valueTupleType;
    private readonly INamedTypeSymbol? debugType;

    private SymbolFindings(Document document, SemanticModel model)
    {
        this.document = document;
        this.model = model;
        expressionType = model.Compilation.GetTypeByMetadataName("System.Linq.Expressions.Expression`1");
        valueTupleType = model.Compilation.GetTypeByMetadataName("System.ValueTuple");
        debugType = model.Compilation.GetTypeByMetadataName("System.Diagnostics.Debug");
    }

    public static Finding[] Collect(Document document, SemanticModel model)
    {
        var walk = new SymbolFindings(document, model);
        foreach (var node in document.Tree.GetRoot().DescendantNodes())
        {
            walk.Visit(node);
        }

        return walk.result.ToArray();
    }

    private void Add(string kind, TextSpan span, string text = "", TextSpan? replaces = null)
    {
        var finding = new Finding
        {
            Kind = kind,
            Start = document.Offsets.At(span.Start),
            Length = document.Offsets.Width(span),
            Text = text,
        };
        if (replaces is { } r)
        {
            finding.FixStart = document.Offsets.At(r.Start);
            finding.FixLength = document.Offsets.Width(r);
        }

        result.Add(finding);
    }

    private void Visit(SyntaxNode node)
    {
        switch (node)
        {
            case BaseExpressionSyntax b:
                BaseCall(b);
                break;
            case ObjectCreationExpressionSyntax or ImplicitObjectCreationExpressionSyntax:
                DefaultConstructor((BaseObjectCreationExpressionSyntax)node);
                if (node is ObjectCreationExpressionSyntax creation)
                {
                    CheckValueTuple(creation.Type, creation.Span, creation);
                }

                break;
            case UsingDirectiveSyntax u:
                UnqualifiedUsing(u);
                break;
            case InvocationExpressionSyntax i:
                Invocation(i);
                break;
            case DefaultExpressionSyntax d:
                CheckValueTuple(d.Type, null, d);
                break;
            case CastExpressionSyntax c:
                CheckValueTuple(c.Type, null, c);
                break;
            case MethodDeclarationSyntax m:
                Signature(m, m.ReturnType, m.ParameterList, m.Modifiers.Any(SyntaxKind.OverrideKeyword));
                break;
            case ConstructorDeclarationSyntax c:
                Signature(c, null, c.ParameterList, false);
                break;
            case ConversionOperatorDeclarationSyntax o:
                Signature(o, o.Type, o.ParameterList, false);
                break;
            case DelegateDeclarationSyntax d:
                Signature(d, d.ReturnType, d.ParameterList, false);
                break;
            case PropertyDeclarationSyntax p:
                Signature(p, p.Type, null, false);
                break;
            case IndexerDeclarationSyntax x:
                Signature(x, x.Type, null, false);
                break;
            case FieldDeclarationSyntax f:
                CheckValueTuple(f.Declaration.Type, null, f);
                break;
            case ParenthesizedLambdaExpressionSyntax l:
                CheckValueTuple(l.ReturnType, null, l);
                foreach (var p in l.ParameterList.Parameters)
                {
                    CheckValueTuple(p.Type, null, l);
                }

                break;
        }

        if (node is BaseTypeDeclarationSyntax or DelegateDeclarationSyntax)
        {
            InheritDoc(node, true);
        }
        else if (node is ConstructorDeclarationSyntax or DestructorDeclarationSyntax or EventDeclarationSyntax
                 or MethodDeclarationSyntax or PropertyDeclarationSyntax or EventFieldDeclarationSyntax
                 or FieldDeclarationSyntax or IndexerDeclarationSyntax or OperatorDeclarationSyntax
                 or ConversionOperatorDeclarationSyntax)
        {
            InheritDoc(node, false);
        }
    }

    // SA1100 -------------------------------------------------------------------------------------

    private void BaseCall(BaseExpressionSyntax b)
    {
        var parent = b.Parent;
        if (parent is null || model.GetSymbolInfo(parent).Symbol is not { } target)
        {
            return;
        }

        ExpressionSyntax speculative;
        if (parent is MemberAccessExpressionSyntax access)
        {
            // The whole call, or overload resolution fails.
            speculative = access.WithExpression(SyntaxFactory.ThisExpression());
            if (access.Parent is InvocationExpressionSyntax invocation)
            {
                speculative = invocation.WithExpression(speculative);
            }
        }
        else if (parent is ElementAccessExpressionSyntax element)
        {
            speculative = element.WithExpression(SyntaxFactory.ThisExpression());
        }
        else
        {
            return;
        }

        var found = model.GetSpeculativeSymbolInfo(parent.SpanStart, speculative, SpeculativeBindingOption.BindAsExpression);
        if (SymbolEqualityComparer.Default.Equals(target, found.Symbol))
        {
            Add("base_call", b.Span, "this");
        }
    }

    // SA1129 -------------------------------------------------------------------------------------

    private void DefaultConstructor(BaseObjectCreationExpressionSyntax node)
    {
        var operation = model.GetOperation(node);
        ITypeSymbol? type;
        switch (operation)
        {
            case IObjectCreationOperation o:
                type = o.Constructor?.ContainingType ?? o.Type;
                if (!o.Arguments.IsEmpty || o.Initializer is not null || o.Constructor is { IsImplicitlyDeclared: false })
                {
                    return;
                }

                break;
            case ITypeParameterObjectCreationOperation t:
                type = t.Type;
                if (t.Initializer is not null)
                {
                    return;
                }

                break;
            default:
                return;
        }

        if (type is null || type.IsReferenceType || type is ITypeParameterSymbol { HasValueTypeConstraint: false })
        {
            return;
        }

        Add("default_value_constructor", node.Span, node is ObjectCreationExpressionSyntax e ? Replacement(e, type as INamedTypeSymbol) : "");
    }

    private static string Replacement(ObjectCreationExpressionSyntax node, INamedTypeSymbol? type)
    {
        var text = node.Type.ToString().Trim();
        var inParameterDefault = node.Parent?.Parent is ParameterSyntax { Parent.Parent: BaseMethodDeclarationSyntax };
        var native = text is "nint" or "nuint";
        var known = type?.ToDisplayString() is "System.Threading.CancellationToken" or "System.Guid"
            || type?.SpecialType is SpecialType.System_IntPtr or SpecialType.System_UIntPtr;
        if (known && !inParameterDefault && !native)
        {
            var member = type!.ToDisplayString() switch
            {
                "System.Threading.CancellationToken" => "None",
                "System.Guid" => "Empty",
                _ => "Zero",
            };
            return $"{text}.{member}";
        }

        if (type is { TypeKind: TypeKind.Enum })
        {
            var zero = type.GetMembers().OfType<IFieldSymbol>().Where(f => f.ConstantValue?.Equals(0) == true).ToList();
            if (zero.Count == 1)
            {
                return $"{text}.{zero[0].Name}";
            }
        }

        return $"default({text})";
    }

    // SA1135 -------------------------------------------------------------------------------------

    private void UnqualifiedUsing(UsingDirectiveSyntax u)
    {
        if (u.Parent is not BaseNamespaceDeclarationSyntax ns
            || u.Name is not { } name
            || name.DescendantNodesAndSelf().OfType<AliasQualifiedNameSyntax>().Any(a => a.Alias.Identifier.IsKind(SyntaxKind.GlobalKeyword)))
        {
            return;
        }

        var symbol = model.GetSymbolInfo(name).Symbol;
        if (symbol is null)
        {
            return;
        }

        if (symbol is INamedTypeSymbol { IsTupleType: true } tuple)
        {
            symbol = tuple.TupleUnderlyingType ?? symbol;
        }

        var qualified = symbol.ToDisplayString(Qualified);
        var written = string.Concat(name.ToString().Where(c => !char.IsWhiteSpace(c)));
        var first = name.DescendantNodesAndSelf().OfType<IdentifierNameSyntax>().FirstOrDefault();
        if (string.Concat(qualified.Where(c => !char.IsWhiteSpace(c))) == written || (first is not null && model.GetAliasInfo(first) is not null))
        {
            return;
        }

        var report = symbol.Kind == SymbolKind.Namespace
            || (symbol.Kind == SymbolKind.NamedType && ns.Name.ToString() != symbol.ContainingNamespace.ToString());
        if (report)
        {
            Add("unqualified_using", u.Span, written.Contains('<') ? "" : qualified, name.Span);
        }
    }

    // SA1141 and SA1414 --------------------------------------------------------------------------

    private bool InExpressionTree(SyntaxNode node)
    {
        if (expressionType is null)
        {
            return false;
        }

        foreach (var lambda in node.AncestorsAndSelf().OfType<AnonymousFunctionExpressionSyntax>())
        {
            if (SymbolEqualityComparer.Default.Equals(model.GetTypeInfo(lambda).ConvertedType?.OriginalDefinition, expressionType))
            {
                return true;
            }
        }

        return false;
    }

    /// <summary>Reports ValueTuple types in `type`, at `where` when given (the whole creation expression).</summary>
    private void CheckValueTuple(TypeSyntax? type, TextSpan? where, SyntaxNode context)
    {
        switch (type)
        {
            case TupleTypeSyntax tuple:
                foreach (var e in tuple.Elements)
                {
                    CheckValueTuple(e.Type, where, context);
                }

                break;
            case QualifiedNameSyntax q:
                CheckValueTuple(q.Right, where ?? q.Span, context, q);
                break;
            case GenericNameSyntax g:
                CheckValueTuple(g, where, context, g);
                break;
        }
    }

    private void CheckValueTuple(TypeSyntax type, TextSpan? where, SyntaxNode context, TypeSyntax whole)
    {
        if (type is not GenericNameSyntax g)
        {
            CheckValueTuple(type, where, context);
            return;
        }

        if (g.Identifier.ValueText == "ValueTuple"
            && model.GetSymbolInfo(g).Symbol is INamedTypeSymbol { IsTupleType: true } t
            && t.TupleElements.Length > 1
            && !InExpressionTree(context))
        {
            var text = where is null
                ? "(" + string.Join(", ", g.TypeArgumentList.Arguments.Select(a => a.ToString().Trim())) + ")"
                : "";
            Add("value_tuple", where ?? whole.Span, text);
            return;
        }

        foreach (var a in g.TypeArgumentList.Arguments)
        {
            CheckValueTuple(a, where, context);
        }
    }

    private void Invocation(InvocationExpressionSyntax i)
    {
        DebugMessage(i);
        if (i.ArgumentList.Arguments.Count >= 2
            && i.Expression is MemberAccessExpressionSyntax { Name.Identifier.ValueText: "Create" } access
            && model.GetSymbolInfo(i).Symbol is IMethodSymbol m
            && SymbolEqualityComparer.Default.Equals(m.ContainingType, valueTupleType)
            && !InExpressionTree(i))
        {
            // The report is on the member access; the fix replaces the whole call.
            var text = "(" + string.Join(", ", i.ArgumentList.Arguments.Select(a => a.ToString().Trim())) + ")";
            Add("value_tuple", access.Span, text, i.Span);
        }
    }

    private void Signature(SyntaxNode declaration, TypeSyntax? returns, ParameterListSyntax? parameters, bool isOverride)
    {
        var types = new List<TypeSyntax>();
        if (returns is not null)
        {
            types.Add(returns);
        }

        types.AddRange(parameters?.Parameters.Select(p => p.Type).OfType<TypeSyntax>() ?? []);
        foreach (var type in types)
        {
            CheckValueTuple(type, null, declaration);
        }

        if (isOverride)
        {
            return;
        }

        List<TupleElementSyntax>? unnamed = null;
        foreach (var type in types)
        {
            Unnamed(type, ref unnamed);
        }

        if (unnamed is null)
        {
            return;
        }

        var symbol = model.GetDeclaredSymbol(declaration);
        if (symbol is not null && Inherits(symbol))
        {
            return;
        }

        foreach (var e in unnamed)
        {
            Add("unnamed_tuple_element", e.Span);
        }
    }

    private static void Unnamed(TypeSyntax type, ref List<TupleElementSyntax>? found)
    {
        switch (type)
        {
            case TupleTypeSyntax tuple:
                foreach (var e in tuple.Elements)
                {
                    Unnamed(e.Type, ref found);
                    if (e.Identifier.IsKind(SyntaxKind.None))
                    {
                        (found ??= []).Add(e);
                    }
                }

                break;
            case QualifiedNameSyntax q:
                Unnamed(q.Right, ref found);
                break;
            case GenericNameSyntax g:
                foreach (var a in g.TypeArgumentList.Arguments)
                {
                    Unnamed(a, ref found);
                }

                break;
        }
    }

    // SA1405 and SA1406 --------------------------------------------------------------------------

    private void DebugMessage(InvocationExpressionSyntax i)
    {
        var name = i.Expression switch
        {
            MemberAccessExpressionSyntax m => m.Name.Identifier.ValueText,
            IdentifierNameSyntax n => n.Identifier.ValueText,
            _ => null,
        };
        var (kind, index) = name switch
        {
            "Assert" => ("debug_assert_message", 1),
            "Fail" => ("debug_fail_message", 0),
            _ => ("", 0),
        };
        if (kind == "" || model.GetSymbolInfo(i).Symbol is not IMethodSymbol m2
            || !SymbolEqualityComparer.Default.Equals(m2.ContainingType, debugType) || m2.Name != name)
        {
            return;
        }

        var arguments = i.ArgumentList.Arguments;
        if (arguments.Count <= index)
        {
            Add(kind, i.Span);
            return;
        }

        var constant = model.GetConstantValue(arguments[index].Expression);
        if (constant.HasValue && string.IsNullOrWhiteSpace(constant.Value as string))
        {
            Add(kind, i.Span);
        }
    }

    // SA1648 -------------------------------------------------------------------------------------

    private void InheritDoc(SyntaxNode declaration, bool typeLike)
    {
        if (typeLike && declaration is BaseTypeDeclarationSyntax { BaseList.Types.Count: > 0 })
        {
            return;
        }

        if (!typeLike && declaration is MemberDeclarationSyntax member
            && member.Modifiers.Any(SyntaxKind.OverrideKeyword))
        {
            return;
        }

        var doc = declaration.GetLeadingTrivia().Select(t => t.GetStructure()).OfType<DocumentationCommentTriviaSyntax>().LastOrDefault();
        var element = doc?.Content.Select(Element).FirstOrDefault(e => e is not null && !HasCref(e));
        if (element is null)
        {
            return;
        }

        if (!typeLike)
        {
            var symbol = model.GetDeclaredSymbol(declaration);
            if (symbol is null && declaration is EventFieldDeclarationSyntax { Declaration.Variables: [var first, ..] })
            {
                symbol = model.GetDeclaredSymbol(first);
            }

            if (declaration is ConstructorDeclarationSyntax && symbol is IMethodSymbol { ContainingType.BaseType: { } baseType } ctor
                && baseType.Constructors.Any(b => b.Parameters.Length == ctor.Parameters.Length
                    && b.Parameters.Zip(ctor.Parameters).All(p => SymbolEqualityComparer.Default.Equals(p.First.Type, p.Second.Type))))
            {
                return;
            }

            if (symbol is not null && Inherits(symbol))
            {
                return;
            }
        }

        Add("inheritdoc", element.Span);
    }

    /// <summary>Implements an interface member, implicitly or explicitly.</summary>
    private static bool Inherits(ISymbol symbol) =>
        FileQueries.Implements(symbol)
        || symbol is IMethodSymbol { ExplicitInterfaceImplementations.Length: > 0 }
            or IPropertySymbol { ExplicitInterfaceImplementations.Length: > 0 }
            or IEventSymbol { ExplicitInterfaceImplementations.Length: > 0 };

    private static XmlNodeSyntax? Element(XmlNodeSyntax node) => node switch
    {
        XmlElementSyntax { StartTag.Name.LocalName.ValueText: "inheritdoc" } => node,
        XmlEmptyElementSyntax { Name.LocalName.ValueText: "inheritdoc" } => node,
        _ => null,
    };

    private static bool HasCref(XmlNodeSyntax node) => node switch
    {
        XmlElementSyntax e => e.StartTag.Attributes.Any(SyntaxKind.XmlCrefAttribute),
        XmlEmptyElementSyntax e => e.Attributes.Any(SyntaxKind.XmlCrefAttribute),
        _ => false,
    };
}
