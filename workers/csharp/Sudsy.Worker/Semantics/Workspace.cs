using System.Reflection;
using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Sudsy.Worker.Mapping;

namespace Sudsy.Worker.Semantics;

/// <summary>One file of the workspace and what is derived from its current text.</summary>
internal sealed class Document(string id, ulong version, string text, SyntaxTree tree)
{
    public string Id { get; } = id;
    public ulong Version { get; } = version;
    public string Text { get; } = text;
    public SyntaxTree Tree { get; } = tree;
    public Utf8Offsets Offsets { get; } = new(text);

    // Per compilation: dropped when any document changes.
    public SemanticModel? Model { get; set; }
    public IReadOnlyList<Diagnostic>? Diagnostics { get; set; }
}

/// <summary>
/// The Roslyn workspace: the C# files under the opened project, solution or directory paths plus
/// the text the editor or CLI sends, as one compilation against the runtime's reference
/// assemblies. Not MSBuildWorkspace: project references, NuGet packages and conditional
/// compilation are not evaluated (see docs/decisions.md, M5).
/// </summary>
internal sealed class Workspace
{
    private static readonly CSharpParseOptions ParseOptions = new(LanguageVersion.Latest);

    private static readonly CSharpCompilationOptions CompilationOptions =
        new(OutputKind.DynamicallyLinkedLibrary, allowUnsafe: true);

    private static readonly Lazy<MetadataReference[]> Framework = new(LoadFramework);

    private static readonly StringComparer PathComparer =
        OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal;

    private readonly Dictionary<string, Document> _documents = new(PathComparer);
    private readonly HashSet<string> _fromDisk = new(PathComparer);
    private string[] _roots = [];
    private bool _loaded = true;
    private CSharpCompilation? _compilation;

    public static string Normalize(string id)
    {
        try
        {
            return Path.GetFullPath(id);
        }
        catch (Exception e) when (e is ArgumentException or NotSupportedException or PathTooLongException)
        {
            return id;
        }
    }

    /// <summary>Remembers the roots; their files are read on the first query.</summary>
    public void Open(IEnumerable<string> paths)
    {
        foreach (var id in _fromDisk)
        {
            _documents.Remove(id);
        }

        _fromDisk.Clear();
        _roots = paths.Select(Normalize).ToArray();
        _loaded = _roots.Length == 0;
        _compilation = null;
    }

    public void Update(string id, ulong version, string text)
    {
        id = Normalize(id);
        if (_documents.TryGetValue(id, out var old) && old.Text == text)
        {
            return;
        }

        var tree = CSharpSyntaxTree.ParseText(text, ParseOptions, path: id);
        var document = new Document(id, version, text, tree);
        _documents[id] = document;
        _fromDisk.Remove(id);
        if (_compilation is { } c)
        {
            _compilation = old is null ? c.AddSyntaxTrees(tree) : c.ReplaceSyntaxTree(old.Tree, tree);
            foreach (var d in _documents.Values)
            {
                d.Model = null;
                d.Diagnostics = null;
            }
        }
    }

    /// <summary>The document and its semantic model; null when the file is unknown.</summary>
    public (Document Document, SemanticModel Model)? Get(string id)
    {
        EnsureLoaded();
        if (!_documents.TryGetValue(Normalize(id), out var document))
        {
            return null;
        }

        var compilation = Compilation();
        document.Model ??= compilation.GetSemanticModel(document.Tree);
        return (document, document.Model);
    }

    public IReadOnlyList<Diagnostic> Diagnostics(Document document, SemanticModel model) =>
        document.Diagnostics ??= model.GetDiagnostics();

    public IEnumerable<Document> Documents => _documents.Values;

    private CSharpCompilation Compilation() =>
        _compilation ??= CSharpCompilation.Create(
            "sudsy",
            _documents.Values.Select(d => d.Tree),
            Framework.Value,
            CompilationOptions);

    private void EnsureLoaded()
    {
        if (_loaded)
        {
            return;
        }

        _loaded = true;
        var files = _roots.SelectMany(Sources).Distinct(PathComparer).Where(f => !_documents.ContainsKey(f)).ToArray();
        var loaded = new Document?[files.Length];
        Parallel.For(0, files.Length, i =>
        {
            try
            {
                var text = new UTF8Encoding(false, true).GetString(File.ReadAllBytes(files[i]));
                if (text.StartsWith('﻿'))
                {
                    text = text[1..];
                }

                loaded[i] = new Document(files[i], 0, text, CSharpSyntaxTree.ParseText(text, ParseOptions, path: files[i]));
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException or DecoderFallbackException)
            {
                // An unreadable file is left out of the compilation.
            }
        });
        foreach (var d in loaded.OfType<Document>())
        {
            _documents[d.Id] = d;
            _fromDisk.Add(d.Id);
        }

        _compilation = null;
    }

    private static readonly string[] Skipped = ["bin", "obj", "node_modules", ".git"];

    /// <summary>The C# files a root stands for: a directory, the directory of a project, or the
    /// directories of the projects listed in a solution.</summary>
    private static IEnumerable<string> Sources(string root)
    {
        if (File.Exists(root))
        {
            var extension = Path.GetExtension(root).ToLowerInvariant();
            var dir = Path.GetDirectoryName(root) ?? ".";
            if (extension is ".sln" or ".slnx")
            {
                var text = File.ReadAllText(root);
                var projects = System.Text.RegularExpressions.Regex
                    .Matches(text, "[\"'](?<p>[^\"']+?\\.csproj)[\"']")
                    .Select(m => Path.GetFullPath(Path.Combine(dir, m.Groups["p"].Value.Replace('\\', Path.DirectorySeparatorChar))));
                return projects.SelectMany(p => Sources(p));
            }

            return extension == ".cs" ? [root] : Sources(dir);
        }

        return Directory.Exists(root) ? Walk(root) : [];
    }

    private static IEnumerable<string> Walk(string dir)
    {
        foreach (var file in Directory.EnumerateFiles(dir, "*.cs"))
        {
            yield return file;
        }

        foreach (var sub in Directory.EnumerateDirectories(dir))
        {
            if (!Skipped.Contains(Path.GetFileName(sub), StringComparer.OrdinalIgnoreCase))
            {
                foreach (var file in Walk(sub))
                {
                    yield return file;
                }
            }
        }
    }

    private static MetadataReference[] LoadFramework()
    {
        var references = new List<MetadataReference>();
        foreach (var file in Directory.EnumerateFiles(System.Runtime.InteropServices.RuntimeEnvironment.GetRuntimeDirectory(), "*.dll"))
        {
            try
            {
                AssemblyName.GetAssemblyName(file);
                references.Add(MetadataReference.CreateFromFile(file));
            }
            catch (Exception e) when (e is BadImageFormatException or FileLoadException or IOException)
            {
                // A native library next to the managed ones.
            }
        }

        return references.ToArray();
    }
}
