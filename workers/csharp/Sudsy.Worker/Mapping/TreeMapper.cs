using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.Text;

namespace Sudsy.Worker.Mapping;

/// <summary>
/// Roslyn tree to our grammar's tree. The only code that knows Roslyn tree shapes.
/// Re-attaches trivia to the shared rule (crates/core-syntax/src/trivia.rs):
/// between two tokens, the trailing trivia of the first is the run up to and
/// including the first trivia that ends with a line break (the whole run if none);
/// the rest is leading trivia of the second.
/// </summary>
public static class TreeMapper
{
    public static FlatTree Map(SyntaxNode root, string sourceText)
    {
        var text = new Source(sourceText);
        var tokens = new List<SyntaxToken>();
        foreach (var t in root.DescendantTokens())
        {
            // Missing tokens without text or trivia are not part of the source.
            if (t.FullSpan.Length > 0 || t.RawKind == (int)Microsoft.CodeAnalysis.CSharp.SyntaxKind.EndOfFileToken)
            {
                tokens.Add(t);
            }
        }

        var cuts = ComputeCuts(tokens, text);
        var tree = new FlatTree();
        var stack = new Stack<Frame>();
        stack.Push(new Frame(root.ChildNodesAndTokens().GetEnumerator(), tree.Add(GrammarKinds.Of(root.RawKind), 0)));
        var tokenIndex = 0;

        while (stack.Count > 0)
        {
            var frame = stack.Pop();
            if (!frame.Children.MoveNext())
            {
                tree.SetValue(frame.Slot, frame.Count);
                continue;
            }

            var child = frame.Children.Current;
            if (child.IsNode)
            {
                frame.Count++;
                var node = child.AsNode()!;
                stack.Push(frame);
                stack.Push(new Frame(node.ChildNodesAndTokens().GetEnumerator(), tree.Add(GrammarKinds.Of(node.RawKind), 0)));
                continue;
            }

            if (tokenIndex < tokens.Count && tokens[tokenIndex] == child.AsToken())
            {
                frame.Count += 1 + EmitToken(tree, tokens, cuts, tokenIndex, text);
                tokenIndex++;
            }

            stack.Push(frame);
        }

        if (tokenIndex != tokens.Count)
        {
            throw new MappingException("token walk and token list disagree");
        }

        return tree;
    }

    private struct Frame(ChildSyntaxList.Enumerator children, int slot)
    {
        public ChildSyntaxList.Enumerator Children = children;
        public readonly int Slot = slot;
        public uint Count;
    }

    /// <summary>For each token, how many of (its trailing + next token leading) trivia stay trailing.</summary>
    private static int[] ComputeCuts(List<SyntaxToken> tokens, Source text)
    {
        var cuts = new int[tokens.Count];
        for (var i = 0; i < tokens.Count; i++)
        {
            var trailing = tokens[i].TrailingTrivia;
            var next = i + 1 < tokens.Count ? tokens[i + 1].LeadingTrivia : default;
            var total = trailing.Count + next.Count;
            var cut = total;
            for (var j = 0; j < total; j++)
            {
                if (EndsLine(text, Pick(trailing, next, j)))
                {
                    cut = j + 1;
                    break;
                }
            }

            cuts[i] = cut;
        }

        return cuts;
    }

    private static SyntaxTrivia Pick(SyntaxTriviaList trailing, SyntaxTriviaList next, int j) =>
        j < trailing.Count ? trailing[j] : next[j - trailing.Count];

    private static bool EndsLine(Source text, SyntaxTrivia trivia)
    {
        var span = trivia.FullSpan;
        return span.Length > 0 && IsLineBreak(text.Text[span.End - 1]);
    }

    private static bool IsLineBreak(char c) => (int)c is 0x0A or 0x0D or 0x85 or 0x2028 or 0x2029;

    /// <summary>Emits leading trivia, the token and trailing trivia; returns the number of trivia siblings added.</summary>
    private static uint EmitToken(FlatTree tree, List<SyntaxToken> tokens, int[] cuts, int i, Source text)
    {
        uint trivia = 0;
        var token = tokens[i];
        if (i == 0)
        {
            foreach (var t in token.LeadingTrivia)
            {
                EmitTrivia(tree, t, text);
                trivia++;
            }
        }
        else
        {
            var prevTrailing = tokens[i - 1].TrailingTrivia;
            var total = prevTrailing.Count + token.LeadingTrivia.Count;
            for (var j = cuts[i - 1]; j < total; j++)
            {
                EmitTrivia(tree, Pick(prevTrailing, token.LeadingTrivia, j), text);
                trivia++;
            }
        }

        tree.Add(GrammarKinds.Of(token.RawKind), (uint)Utf8Length(text, token.Span));

        var trailing = token.TrailingTrivia;
        var nextLeading = i + 1 < tokens.Count ? tokens[i + 1].LeadingTrivia : default;
        for (var j = 0; j < cuts[i]; j++)
        {
            EmitTrivia(tree, Pick(trailing, nextLeading, j), text);
            trivia++;
        }

        return trivia;
    }

    private static void EmitTrivia(FlatTree tree, SyntaxTrivia trivia, Source text) =>
        tree.Add(GrammarKinds.Of(trivia.RawKind), (uint)Utf8Length(text, trivia.FullSpan));

    private static int Utf8Length(Source text, TextSpan span) =>
        text.Ascii ? span.Length : Encoding.UTF8.GetByteCount(text.Text.AsSpan(span.Start, span.Length));
}

/// <summary>The source text, with a flag that makes UTF-8 widths free for pure ASCII input.</summary>
internal readonly struct Source(string text)
{
    public string Text { get; } = text;

    public bool Ascii { get; } = System.Text.Ascii.IsValid(text);
}
