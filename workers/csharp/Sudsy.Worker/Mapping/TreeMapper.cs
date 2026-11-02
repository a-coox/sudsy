using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.Text;
using CS = Microsoft.CodeAnalysis.CSharp;

namespace Sudsy.Worker.Mapping;

/// <summary>
/// Roslyn red tree to our flat tree. This is the only code that knows Roslyn's tree shapes.
///
/// * Trivia are re-attached to the shared rule (trailing: up to and including the first trivia that
///   ends a line; leading: the rest) and emitted as siblings of their token.
/// * Doc comments and directives are structured trivia: nodes, mapped recursively as a scope of
///   their own. Disabled text, whitespace, comments and line ends stay plain trivia tokens.
/// * Missing tokens become zero-width <c>ERROR</c> nodes; skipped tokens become <c>ERROR</c> nodes
///   holding the skipped tokens (with their own trivia), so every token is in the tree.
/// </summary>
public static class TreeMapper
{
    public static FlatTree Map(SyntaxNode root, string text) => new Mapper(text, forceGeneral: false).Run(root);

    /// <summary>Same output through the general routine for every token; used by tests to cross-check the fast path.</summary>
    internal static FlatTree MapGeneral(SyntaxNode root, string text) => new Mapper(text, forceGeneral: true).Run(root);
}

internal sealed class Mapper
{
    private struct Frame
    {
        public ChildSyntaxList.Enumerator Children;
        public int Slot;
        public uint Count;
    }

    /// <summary>State of one scope: the file, or one structured trivia.</summary>
    private sealed class Scope
    {
        public readonly List<SyntaxTrivia> Carry = [];
        // Scratch for the general routine; per scope because structured trivia are mapped inside it.
        public readonly List<Item> Items = [];
        public readonly List<int> Lexemes = [];
        public readonly List<int> Cuts = [];
        /// <summary>Leading trivia of the next token already emitted as trailing trivia of the previous one.</summary>
        public int Consumed;
        public int Start;
        /// <summary>Index of the innermost open frame while a token is visited.</summary>
        public int Top;
        public int End;
        public Frame[] Frames = new Frame[32];
    }

    private readonly string _text;
    private readonly int[]? _byteOffsets;
    private readonly bool _forceGeneral;
    private readonly FlatTree _tree;
    private readonly List<Scope> _scopes = [];
    /// <summary>The text has disabled text or skipped tokens: check every token's trivia for the latter.</summary>
    private bool _scanForSkipped;

    public Mapper(string text, bool forceGeneral)
    {
        _text = text;
        _forceGeneral = forceGeneral;
        _byteOffsets = System.Text.Ascii.IsValid(text) ? null : Utf8Offsets(text);
        _tree = new FlatTree((text.Length / 3) + 64);
    }

    public FlatTree Run(SyntaxNode root)
    {
        _scanForSkipped = root.ContainsSkippedText && root.ContainsDiagnostics;
        RunScope(root, 0);
        return _tree;
    }

    private static int[] Utf8Offsets(string text)
    {
        var offsets = new int[text.Length + 1];
        var bytes = 0;
        for (var i = 0; i < text.Length; i++)
        {
            var c = text[i];
            if (c < 0x80)
            {
                bytes += 1;
            }
            else if (c < 0x800)
            {
                bytes += 2;
            }
            else if (char.IsHighSurrogate(c) && i + 1 < text.Length && char.IsLowSurrogate(text[i + 1]))
            {
                bytes += 4;
                offsets[i + 1] = bytes;
                i++;
            }
            else
            {
                bytes += 3;
            }

            offsets[i + 1] = bytes;
        }

        return offsets;
    }

    private uint Width(TextSpan span) => _byteOffsets is null
        ? (uint)span.Length
        : (uint)(_byteOffsets[span.End] - _byteOffsets[span.Start]);

    private Scope ScopeAt(int depth)
    {
        while (_scopes.Count <= depth)
        {
            _scopes.Add(new Scope());
        }

        var scope = _scopes[depth];
        scope.Carry.Clear();
        scope.Consumed = 0;
        return scope;
    }

    /// <summary>Emits <paramref name="root"/> and everything below it as one element.</summary>
    private void RunScope(SyntaxNode root, int depth)
    {
        var scope = ScopeAt(depth);
        scope.Start = root.FullSpan.Start;
        scope.End = root.FullSpan.End;
        var frames = scope.Frames;
        var top = 0;
        frames[0] = new Frame
        {
            Children = root.ChildNodesAndTokens().GetEnumerator(),
            Slot = _tree.Add(GrammarKinds.Of(root.RawKind), 0),
        };

        while (top >= 0)
        {
            ref var frame = ref frames[top];
            if (!frame.Children.MoveNext())
            {
                _tree.SetValue(frame.Slot, frame.Count);
                top--;
                continue;
            }

            var child = frame.Children.Current;
            if (child.IsNode)
            {
                frame.Count++;
                var node = child.AsNode()!;
                if (++top == frames.Length)
                {
                    Array.Resize(ref frames, frames.Length * 2);
                    scope.Frames = frames;
                }

                frames[top] = new Frame
                {
                    Children = node.ChildNodesAndTokens().GetEnumerator(),
                    Slot = _tree.Add(GrammarKinds.Of(node.RawKind), 0),
                };
                continue;
            }

            scope.Top = top;
            var token = child.AsToken();
            var added = _forceGeneral || token.IsMissing || (_scanForSkipped && HasSkipped(token))
                ? VisitGeneral(token, scope, depth)
                : VisitSimple(token, scope, depth);
            frames[top].Count += added;
        }
    }

    private static bool HasSkipped(SyntaxToken token)
    {
        foreach (var t in token.LeadingTrivia)
        {
            if (t.RawKind == (int)CS.SyntaxKind.SkippedTokensTrivia)
            {
                return true;
            }
        }

        foreach (var t in token.TrailingTrivia)
        {
            if (t.RawKind == (int)CS.SyntaxKind.SkippedTokensTrivia)
            {
                return true;
            }
        }

        return false;
    }

    // ---------------------------------------------------------------- fast routine

    /// <summary>A present token with plain trivia only. Returns the number of elements added.</summary>
    private uint VisitSimple(SyntaxToken token, Scope scope, int depth)
    {
        uint added = 0;
        foreach (var t in scope.Carry)
        {
            added += EmitTrivia(t, depth);
        }

        scope.Carry.Clear();

        var skip = scope.Consumed;
        scope.Consumed = 0;
        foreach (var t in token.LeadingTrivia)
        {
            if (skip > 0)
            {
                skip--;
                continue;
            }

            added += EmitTrivia(t, depth);
        }

        _tree.Add(GrammarKinds.Of(token.RawKind), Width(token.Span));
        added++;

        var open = true;
        foreach (var t in token.TrailingTrivia)
        {
            if (!open)
            {
                scope.Carry.Add(t);
                continue;
            }

            added += EmitTrivia(t, depth);
            open = !EndsLine(t);
        }

        if (open)
        {
            added += ExtendRun(token, scope, depth);
        }

        return added;
    }

    // ---------------------------------------------------------------- general routine

    private enum ItemKind : byte
    {
        Trivia,
        Lexeme,
    }

    private struct Item
    {
        public ItemKind Kind;
        public SyntaxTrivia Trivia;
        public SyntaxToken Token;
        /// <summary>Skipped tokens of one skipped-tokens trivia share a non-zero group.</summary>
        public int Group;
    }

    /// <summary>
    /// Any token: missing ones, and ones with skipped tokens among their trivia. Lays out the token's
    /// stretch of the text as a list of trivia and lexemes (the token itself unless it is missing,
    /// and its skipped tokens), cuts every run between two lexemes by the shared rule, and emits.
    /// </summary>
    private uint VisitGeneral(SyntaxToken token, Scope scope, int depth)
    {
        var items = scope.Items;
        items.Clear();
        foreach (var t in scope.Carry)
        {
            items.Add(new Item { Kind = ItemKind.Trivia, Trivia = t });
        }

        scope.Carry.Clear();
        var group = 0;
        var skip = scope.Consumed;
        scope.Consumed = 0;
        foreach (var t in token.LeadingTrivia)
        {
            if (skip > 0)
            {
                skip--;
                continue;
            }

            AddExpanded(t, scope.Items, ref group);
        }

        var missing = token.IsMissing;
        if (!missing)
        {
            items.Add(new Item { Kind = ItemKind.Lexeme, Token = token });
        }

        foreach (var t in token.TrailingTrivia)
        {
            AddExpanded(t, items, ref group);
        }

        var lexemes = scope.Lexemes;
        lexemes.Clear();
        for (var i = 0; i < items.Count; i++)
        {
            if (items[i].Kind == ItemKind.Lexeme)
            {
                lexemes.Add(i);
            }
        }

        uint added = 0;
        if (lexemes.Count == 0)
        {
            // Only trivia of a missing token: they lead whatever comes next.
            foreach (var item in items)
            {
                scope.Carry.Add(item.Trivia);
            }

            return EmitMissing(missing, ref added);
        }

        // cuts[k]: where lexeme k's trailing trivia end (exclusive); the last run is open when it has no line break.
        var cuts = scope.Cuts;
        cuts.Clear();
        var lastOpen = true;
        for (var k = 0; k < lexemes.Count; k++)
        {
            var from = lexemes[k] + 1;
            var to = k + 1 < lexemes.Count ? lexemes[k + 1] : items.Count;
            var cut = to;
            var broke = false;
            for (var i = from; i < to; i++)
            {
                if (EndsLine(items[i].Trivia))
                {
                    cut = i + 1;
                    broke = true;
                    break;
                }
            }

            cuts.Add(cut);
            if (k + 1 == lexemes.Count)
            {
                lastOpen = !broke;
            }
        }

        // Items after the last cut lead whatever comes next.
        for (var i = cuts[^1]; i < items.Count; i++)
        {
            scope.Carry.Add(items[i].Trivia);
        }

        var inGroup = 0;
        var groupSlot = 0;
        uint groupCount = 0;
        for (var k = 0; k < lexemes.Count; k++)
        {
            var start = k == 0 ? 0 : cuts[k - 1];
            var lexeme = items[lexemes[k]];
            if (lexeme.Group != inGroup)
            {
                // Leaving one group (if any) and entering another happen at the same cut.
                if (inGroup != 0)
                {
                    _tree.SetValue(groupSlot, groupCount);
                    added++;
                }

                inGroup = lexeme.Group;
                if (inGroup != 0)
                {
                    groupSlot = _tree.Add(GrammarKinds.Error, 0);
                    groupCount = 0;
                }
            }

            uint n = 0;
            for (var i = start; i < lexemes[k]; i++)
            {
                n += EmitTrivia(items[i].Trivia, depth);
            }

            _tree.Add(GrammarKinds.Of(lexeme.Token.RawKind), Width(lexeme.Token.Span));
            n++;
            for (var i = lexemes[k] + 1; i < cuts[k]; i++)
            {
                n += EmitTrivia(items[i].Trivia, depth);
            }

            if (inGroup != 0)
            {
                groupCount += n;
            }
            else
            {
                added += n;
            }
        }

        if (inGroup != 0)
        {
            _tree.SetValue(groupSlot, groupCount);
            added++;
        }

        EmitMissing(missing, ref added);
        if (lastOpen)
        {
            added += ExtendRun(token, scope, depth);
        }

        return added;
    }

    private uint EmitMissing(bool missing, ref uint added)
    {
        if (missing)
        {
            _tree.Add(GrammarKinds.Error, 0);
            added++;
        }

        return added;
    }

    /// <summary>Adds a trivia, or the lexemes (with their trivia) of a skipped-tokens trivia.</summary>
    private static void AddExpanded(SyntaxTrivia trivia, List<Item> items, ref int group)
    {
        if (trivia.RawKind != (int)CS.SyntaxKind.SkippedTokensTrivia)
        {
            items.Add(new Item { Kind = ItemKind.Trivia, Trivia = trivia });
            return;
        }

        group++;
        var skipped = (CS.Syntax.SkippedTokensTriviaSyntax)trivia.GetStructure()!;
        foreach (var token in skipped.Tokens)
        {
            foreach (var t in token.LeadingTrivia)
            {
                items.Add(new Item { Kind = ItemKind.Trivia, Trivia = t });
            }

            items.Add(new Item { Kind = ItemKind.Lexeme, Token = token, Group = group });
            foreach (var t in token.TrailingTrivia)
            {
                items.Add(new Item { Kind = ItemKind.Trivia, Trivia = t });
            }
        }
    }

    // ---------------------------------------------------------------- shared

    /// <summary>
    /// The trailing trivia of <paramref name="token"/> ended without a line break, so the run
    /// continues into the leading trivia of the next token up to its first line break. Emits those
    /// as trailing trivia and remembers how many to skip when the next token comes.
    /// </summary>
    private uint ExtendRun(SyntaxToken token, Scope scope, int depth)
    {
        if (!NextMayStartWithTrivia(token, token.FullSpan.End, depth))
        {
            return 0;
        }

        var next = PeekNext(scope, out var sibling);
        if (sibling && (!_scanForSkipped || !HasSkipped(next)))
        {
            // The next token is a sibling (and has no skipped tokens): its leading trivia land beside this token's trailing
            // trivia either way, so there is nothing to move.
            return 0;
        }

        if (next.RawKind == 0 || next.FullSpan.Start < scope.Start || next.FullSpan.End > scope.End || next.IsMissing)
        {
            return 0;
        }

        uint added = 0;
        foreach (var t in next.LeadingTrivia)
        {
            if (t.RawKind == (int)CS.SyntaxKind.SkippedTokensTrivia)
            {
                break;
            }

            added += EmitTrivia(t, depth);
            scope.Consumed++;
            if (EndsLine(t))
            {
                break;
            }
        }

        return added;
    }

    /// <summary>
    /// Whether the next token can start with trivia that the shared rule would make trailing trivia
    /// of <paramref name="token"/>. Roslyn's own trailing trivia already take every space, comment
    /// and line break of code up to the line break, so in code only a directive (<c>#</c>) can be
    /// left. Inside doc comments and directives (depth above zero) the line break after a token in
    /// a tag, a space, or the closing <c>*/</c> is the next token's leading trivia; after the end of
    /// a tag or after text a line break is a token of its own.
    /// </summary>
    private bool NextMayStartWithTrivia(SyntaxToken token, int end, int depth)
    {
        if (end >= _text.Length)
        {
            return false;
        }

        var c = _text[end];
        if (depth == 0)
        {
            // A directive, or a doc comment (Roslyn leaves those to the next token).
            return c == '#'
                || (c == '/' && end + 1 < _text.Length && _text[end + 1] is '/' or '*')
                || (c > 0x7f && char.IsWhiteSpace(c));
        }

        switch (c)
        {
            case '\r':
            case '\n':
                return !EndsTagOrText((CS.SyntaxKind)token.RawKind);
            case ' ':
            case '\t':
            case '/':
            case '*':
            case '#':
                return true;
            default:
                return c > 0x7f && char.IsWhiteSpace(c);
        }
    }

    private static bool EndsTagOrText(CS.SyntaxKind kind) => kind
        is CS.SyntaxKind.GreaterThanToken
        or CS.SyntaxKind.SlashGreaterThanToken
        or CS.SyntaxKind.XmlTextLiteralToken
        or CS.SyntaxKind.XmlEntityLiteralToken
        or CS.SyntaxKind.XmlCDataEndToken
        or CS.SyntaxKind.XmlCommentEndToken
        or CS.SyntaxKind.XmlProcessingInstructionEndToken;

    /// <summary>
    /// The token after the one being visited, found from the open frames: the next sibling at the
    /// innermost level that has one (its first token when it is a node). Cheaper than asking the
    /// token to navigate up the tree. <paramref name="isSibling"/>: it is a child of the same node.
    /// </summary>
    private static SyntaxToken PeekNext(Scope scope, out bool isSibling)
    {
        isSibling = false;
        for (var level = scope.Top; level >= 0; level--)
        {
            var siblings = scope.Frames[level].Children;
            while (siblings.MoveNext())
            {
                var next = siblings.Current;
                if (next.IsToken)
                {
                    isSibling = level == scope.Top;
                    return next.AsToken();
                }

                var first = next.AsNode()!.GetFirstToken(includeZeroWidth: true);
                if (first.RawKind != 0)
                {
                    return first;
                }
            }
        }

        return default;
    }

    private uint EmitTrivia(SyntaxTrivia trivia, int depth)
    {
        var kind = GrammarKinds.Of(trivia.RawKind);
        if (GrammarKinds.IsToken(kind))
        {
            _tree.Add(kind, Width(trivia.FullSpan));
            return 1;
        }

        RunScope(trivia.GetStructure()!, depth + 1);
        return 1;
    }

    private bool EndsLine(SyntaxTrivia trivia)
    {
        switch ((CS.SyntaxKind)trivia.RawKind)
        {
            case CS.SyntaxKind.EndOfLineTrivia:
                return true;
            case CS.SyntaxKind.WhitespaceTrivia:
            case CS.SyntaxKind.SingleLineCommentTrivia:
            case CS.SyntaxKind.MultiLineCommentTrivia:
                return false;
            default:
                var span = trivia.FullSpan;
                return span.Length > 0 && IsLineBreak(_text[span.End - 1]);
        }
    }

    private static bool IsLineBreak(char c) => (int)c is 0x0A or 0x0D or 0x85 or 0x2028 or 0x2029;
}
