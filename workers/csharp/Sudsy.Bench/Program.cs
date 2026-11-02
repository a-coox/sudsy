// Production-path benchmark (NOT PRODUCT CODE). Usage: Sudsy.Bench <corpus dir> <out dir>
//
// Per .cs file: times Roslyn's parse, the mapping walk and the production serializer
// (WireEncoder + MessagePack, as the worker's protocol does). The encoded files are written to
// <out dir> for tools/wire-bench (Rust decode via the production decoder, then rowan build).
using System.Diagnostics;
using System.Text;
using MessagePack;
using Microsoft.CodeAnalysis.CSharp;
using Sudsy.Worker.Mapping;

var corpus = args[0];
var outDir = args[1];
Directory.CreateDirectory(outDir);
var files = Directory.EnumerateFiles(corpus, "*.cs", SearchOption.AllDirectories).Order(StringComparer.Ordinal).ToList();
var texts = files.Select(f => Encoding.UTF8.GetString(File.ReadAllBytes(f))).ToList();
var totalBytes = texts.Sum(t => (long)Encoding.UTF8.GetByteCount(t));
Console.WriteLine($"{files.Count} files, {totalBytes / 1e6:F1} MB");

Stats Run(bool keep)
{
    var s = new Stats();
    var container = keep ? new MemoryStream() : null;
    for (var i = 0; i < files.Count; i++)
    {
        var sw = Stopwatch.StartNew();
        var tree = CSharpSyntaxTree.ParseText(texts[i]);
        var root = tree.GetRoot();
        s.Parse += sw.Elapsed;

        sw.Restart();
        var flat = TreeMapper.Map(root, texts[i]);
        s.Map += sw.Elapsed;
        s.Elements += flat.Count;

        sw.Restart();
        var bytes = MessagePackSerializer.Serialize(WireEncoder.Encode(flat));
        s.Encode += sw.Elapsed;
        s.Size += bytes.Length;
        if (container is not null)
        {
            container.Write(BitConverter.GetBytes(bytes.Length));
            container.Write(bytes);
        }
    }

    if (container is not null)
    {
        File.WriteAllBytes(Path.Combine(outDir, "trees.bin"), container.ToArray());
    }

    return s;
}

Run(keep: false); // warm-up (JIT, caches)
var stats = Run(keep: true);
File.WriteAllLines(Path.Combine(outDir, "files.txt"), files);

Console.WriteLine($"elements: {stats.Elements:N0}");
Console.WriteLine($"roslyn parse: {stats.Parse.TotalMilliseconds:F0} ms");
Console.WriteLine($"mapping walk: {stats.Map.TotalMilliseconds:F0} ms");
Console.WriteLine($"serialize: size {stats.Size / 1e6:F2} MB, {stats.Encode.TotalMilliseconds:F0} ms");

sealed class Stats
{
    public TimeSpan Parse;
    public TimeSpan Map;
    public TimeSpan Encode;
    public long Elements;
    public long Size;
}
