class Strings
{
    string plain = "a\tb\u00e9 \"quoted\"";
    string verbatim = @"C:\temp\ ""x""
second line";
    char c = '\'';
    string raw1 = """one "line" raw""";
    string raw2 = """
        multi
          line "raw"
        """;
    string raw3 = """"nested """ quotes"""";
    string interp = $"x = {x}, y = {y:F2}, z = {z,-8}, {(a ? b : c)} {{literal}}";
    string interpVerbatim = $@"{name}\{path}";
    string interpRaw = $$"""
        {"json": {{value}}}
        """;
    string nested = $"outer {$"inner {deep}"} end";
    byte[] utf8 = "bytes"u8;
    string unicode = "h\u00e9llo \U0001F642 caf\u00e9 \u4e2d\u6587";
    string emoji = "caf\u00e9 \ud83d\ude42";
}
