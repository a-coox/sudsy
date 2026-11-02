// M0 wire-encoding benchmark (NOT PRODUCT CODE). Usage: Sudsy.Bench <corpus dir> <out dir>
//
// For every .cs file: time Roslyn's parse, the mapping walk (common to all options), and the
// serialization of the mapped tree in each candidate encoding:
//   a  flat arrays (custom framing)         b  MessagePack, one array per element
//   c  protobuf packed arrays               d  flat arrays as bin fields in a MessagePack message
// The encoded files are written to <out dir> for tools/wire-bench (Rust decode timings).
using System.Buffers;
using System.Diagnostics;
using System.Text;
using Google.Protobuf;
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

string[] options = ["a", "b", "c", "d"];
var encoders = new Dictionary<string, Func<FlatTree, byte[]>>
{
    ["a"] = FlatArrays,
    ["b"] = MessagePackPerElement,
    ["c"] = ProtobufPacked,
    ["d"] = MessagePackBinFields,
};

Stats Run(bool keep)
{
    var s = new Stats();
    var containers = options.ToDictionary(o => o, _ => keep ? new MemoryStream() : null);
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

        foreach (var o in options)
        {
            sw.Restart();
            var bytes = encoders[o](flat);
            s.Encode[o] = s.Encode.GetValueOrDefault(o) + sw.Elapsed;
            s.Size[o] = s.Size.GetValueOrDefault(o) + bytes.Length;
            if (containers[o] is { } c)
            {
                c.Write(BitConverter.GetBytes(bytes.Length));
                c.Write(bytes);
            }
        }
    }

    foreach (var (o, c) in containers)
    {
        if (c is not null)
        {
            File.WriteAllBytes(Path.Combine(outDir, $"opt-{o}.bin"), c.ToArray());
        }
    }

    return s;
}

Run(keep: false); // warm-up (JIT, caches)
var stats = Run(keep: true);
File.WriteAllLines(Path.Combine(outDir, "files.txt"), files);

Console.WriteLine($"elements: {stats.Elements:N0}");
Console.WriteLine($"roslyn parse: {stats.Parse.TotalMilliseconds:F0} ms");
Console.WriteLine($"mapping walk (all options): {stats.Map.TotalMilliseconds:F0} ms");
foreach (var o in options)
{
    Console.WriteLine($"option {o}: size {stats.Size[o] / 1e6:F2} MB, serialize {stats.Encode[o].TotalMilliseconds:F0} ms");
}

static byte[] FlatArrays(FlatTree t)
{
    var w = WireEncoder.Encode(t);
    var buf = new byte[12 + w.Kinds.Length + 4 + w.Values.Length];
    "SUDS"u8.CopyTo(buf);
    BitConverter.TryWriteBytes(buf.AsSpan(4), 1u);
    BitConverter.TryWriteBytes(buf.AsSpan(8), (uint)t.Count);
    w.Kinds.CopyTo(buf, 12);
    BitConverter.TryWriteBytes(buf.AsSpan(12 + w.Kinds.Length), (uint)w.Values.Length);
    w.Values.CopyTo(buf, 16 + w.Kinds.Length);
    return buf;
}

static byte[] MessagePackPerElement(FlatTree t)
{
    var buffer = new ArrayBufferWriter<byte>(t.Count * 4);
    var w = new MessagePackWriter(buffer);
    w.WriteArrayHeader(t.Count);
    var kinds = t.Kinds;
    var values = t.Values;
    for (var i = 0; i < kinds.Length; i++)
    {
        w.WriteArrayHeader(2);
        w.Write(kinds[i]);
        w.Write(values[i]);
    }

    w.Flush();
    return buffer.WrittenSpan.ToArray();
}

static byte[] ProtobufPacked(FlatTree t)
{
    var kinds = t.Kinds;
    var values = t.Values;
    var kindsLen = 0;
    var valuesLen = 0;
    for (var i = 0; i < kinds.Length; i++)
    {
        kindsLen += CodedOutputStream.ComputeUInt32Size(kinds[i]);
        valuesLen += CodedOutputStream.ComputeUInt32Size(values[i]);
    }

    var buf = new byte[kindsLen + valuesLen + 12];
    var o = new CodedOutputStream(buf);
    o.WriteTag(1, WireFormat.WireType.LengthDelimited);
    o.WriteLength(kindsLen);
    for (var i = 0; i < kinds.Length; i++)
    {
        o.WriteUInt32(kinds[i]);
    }

    o.WriteTag(2, WireFormat.WireType.LengthDelimited);
    o.WriteLength(valuesLen);
    for (var i = 0; i < values.Length; i++)
    {
        o.WriteUInt32(values[i]);
    }

    o.Flush();
    return buf.AsSpan(0, (int)o.Position).ToArray();
}

static byte[] MessagePackBinFields(FlatTree t) => MessagePackSerializer.Serialize(WireEncoder.Encode(t));

sealed class Stats
{
    public TimeSpan Parse;
    public TimeSpan Map;
    public long Elements;
    public Dictionary<string, TimeSpan> Encode = new();
    public Dictionary<string, long> Size = new();
}
