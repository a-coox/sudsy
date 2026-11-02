# csharp/microsoft@1 samples

Source: Microsoft Learn, "Common C# code conventions"
(<https://learn.microsoft.com/dotnet/csharp/fundamentals/coding-style/coding-conventions>),
text retrieved 2026-11-02 from `dotnet/docs`
(`docs/csharp/fundamentals/coding-style/coding-conventions.md`).

* `using-outside-namespace`: the first code block of "Place the using directives outside the
  namespace declaration", unchanged.
* `global-using-in-namespace`: the last code block of the same section, unchanged.
* `file-scoped-namespace`: the guide's `namespace MySampleCode;` and `ExampleClass instance2 = new();`
  lines, wrapped in a class (the guide shows them as fragments).

The guide's other examples are inline snippets (`:::code` includes, not in the page text) or use
a one-line method body (`{ return null; }`), which contradicts its own Allman rule, so they are not
samples. The `.in` files are the `.out` files formatted with `csharp/google@1` (2 spaces, braces on
the line of the header).
