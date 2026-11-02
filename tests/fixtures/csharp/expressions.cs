class Expressions
{
    void M()
    {
        Func<int, int> f = x => x + 1;
        Func<int, int, int> g = (a, b) => a * b;
        Func<int> h = static () => { return 1; };
        Action<int> d = delegate (int i) { };
        var anon = new { A = 1, B = "x", c };
        var obj = new Foo(1, name: "n") { Prop = 1, List = { 1, 2 }, [0] = 5 };
        var tuple = (1, "a", named: 2.5);
        int[] arr = new int[] { 1, 2 };
        int[,] md = new int[2, 3];
        var st = stackalloc int[4];
        var t = typeof(List<>); var s = sizeof(int); var dflt = default(int); int lit = default;
        var n = nameof(Expressions);
        var c = (int)x; var u = (Func<int>)(() => 1);
        var cast = (T)obj; var cond = (a) ? b : c;
        var neg = -x + +y - ~z; var not = !flag; var inc = x++ + --y;
        var deref = *ptr; var addr = &local;
        var aw = await task; var cw = await task.ConfigureAwait(false);
        var ie = obj?.Method()?[0];
        var withIdx = list[1]; var multi = grid[1, 2];
        var gen = Create<int, string>(1);
        var qual = global::System.Math.Max(1, 2);
        var alias = A::B.C;
        var lit2 = 0x1F + 0b1010 + 1_000 + 1.5e3 + 1.5f + 2.5m + 10UL + 'c';
        var thr = x ?? throw new Exception();
        ref int r = ref arr[0];
        var sw = x switch { 1 => "a", _ => "b" };
        var ex = x is { } y2 && y2 > 0;
        var tpl = (x, y) = (y, x);
        var chkd = checked(x + 1) + unchecked(x + 2);
        var curried = a => b => c => a + b + c;
        var neu = new List<int>(capacity: 10);
        var tn = new();
        Foo<int> impl = new(3);
        var arrow = x => x;
        var local = int.Parse("1");
        var lambdaAttr = [Pure] (int x) => x;
        var lambdaTyped = int (int x) => x;
        var fp = &Method;
    }
}
