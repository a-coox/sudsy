class P
{
    string Describe(object o) => o switch
    {
        null => "null",
        int n when n > 10 => "big",
        int or long => "integer",
        string { Length: > 3 and < 10 } s => s,
        Point { X: 0, Y: var y } => $"on axis {y}",
        (1, _) => "tuple",
        [1, .., var last] => $"list {last}",
        [] => "empty",
        not (null or "") => "other",
        _ => "default",
    };

    bool Check(object o)
    {
        if (o is not null and string { Length: 0 }) return true;
        if (o is Point(var a, var b) && a == b) return true;
        if (o is int i && i is >= 1 and <= 9) return true;
        switch (o)
        {
            case int x when x > 0:
            case long:
                return true;
            case string:
            default:
                return false;
        }
    }
}
