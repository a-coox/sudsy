class Chains
{
    void M()
    {
        var q = items.Where(x => x.Active).Select(x => x.Name).OrderBy(n => n).ToList();
        var r = builder.WithName("a").WithValue(1)?.Build()!.Items[0]?.Value;
        var ok = a && b || c && !d || e == f && g != h;
        var sum = a + b * c - d / e % f;
        var shift = a << 2 | b >> 1 & c ^ d;
        var text = "a" + b + "c" + d.ToString() + "e";
        var cond = a ? b : c ? d : e;
        var nul = a ?? b ?? c;
        a ??= b;
        x += y; x -= 1; x <<= 2; x >>>= 1;
        var cmp = a < b && b <= c && c > d && d >= e;
        var asIs = o is string s ? s : o as string;
        var q2 = from x in xs
                 where x > 1
                 let y = x * 2
                 join z in zs on x equals z.Id
                 orderby y descending, x
                 group x by y into g
                 select new { g.Key, Count = g.Count() };
        var range = arr[1..^1]; var idx = arr[^1];
    }
}
