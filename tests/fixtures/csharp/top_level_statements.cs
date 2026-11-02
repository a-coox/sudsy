using System;
using static System.Console;
using Alias = System.Collections.Generic.List<int>;

var numbers = new Alias { 1, 2, 3 };
foreach (var n in numbers)
    WriteLine(n * 2);

int Twice(int x) => x * 2;

if (args.Length == 0)
{
    Console.WriteLine("no args"); // trailing
}
else
{
    return Twice(args.Length);
}

await Task.Delay(1);
return 0;

record Point(int X, int Y);
