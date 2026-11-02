# csharp/google@1 samples

Source: "C# at Google Style Guide" (<https://google.github.io/styleguide/csharpguide.html>), section
"Formatting guidelines / Example", text retrieved 2026-11-02.

Changes to the published example (see `docs/presets.md` for why):

* The example's trailing `//` comments are removed (the guide aligns them in a column; Sudsy
  keeps one space before a trailing comment).
* `_someTable` has a `;` and one element per line (the guide packs `2, 3, 4,` on one line and
  omits the semicolon).
* Left out: `difference2` and `CallWithDelegate(...)` (alternative continuation layouts the guide
  prefers only "if defining after a continuation line break") and the three long-signature
  examples with first-argument alignment or four-space continuation lines. Sudsy lays out an
  argument list that does not fit one item per line, indented one level.

The `.in` file is the `.out` file formatted with `csharp/microsoft@1` (Allman, 4 spaces).
