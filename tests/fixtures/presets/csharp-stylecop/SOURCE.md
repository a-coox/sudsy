# csharp/stylecop samples

`members.in.cs` and `statements.in.cs` are the fixtures `layout_members.cs` and `layout_statements.cs`
of `tests/parity/stylecop/`, which violate the spacing, readability and layout rules the preset
covers. The `.out.cs` files are what `sudsy format` writes with this preset, checked by
`scripts/parity.sh stylecop` (StyleCop.Analyzers 1.2.0-beta.556 reports none of the preset's SA ids
on them; see `docs/parity/stylecop.yaml` for the rules that still differ).
