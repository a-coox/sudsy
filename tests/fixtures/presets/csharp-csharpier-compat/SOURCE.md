# csharp/csharpier-compat samples

`orders.out.cs` is the output of CSharpier 1.3.0 (`dotnet csharpier format`, the version pinned in
`dotnet-tools.json`) for `orders.in.cs`, a hand-written file that covers member bodies, accessors,
a member chain with a lambda argument, an initializer, a switch expression and a mixed `&&` / `||`
chain. The corpus-wide comparison is `crates/lang-csharp/tests/differential.rs`.
