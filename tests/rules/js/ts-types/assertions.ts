a!.b;
^^ no-non-null-assertion
a!!;
^^ no-extra-non-null-assertion
^^ no-non-null-assertion
^^^ no-non-null-assertion
a!?.b;
^^ no-extra-non-null-assertion
^^ no-non-null-assertion
a!?.();
^^ no-extra-non-null-assertion
^^ no-non-null-assertion
a?.b!.c;
^^^^^ no-non-null-assertion
a![0];
^^ no-non-null-assertion
`${a!.b}x`;
// ^^ no-non-null-assertion
