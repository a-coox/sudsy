class C
{
    int[] a = [];
    int[] b = [1, 2, 3];
    List<int> c = [..a, 4, ..b];
    int[][] d = [[1], [2, 3], []];
    Span<int> e = [1, 2,];

    void M(IEnumerable<int> xs)
    {
        Take([.. xs, 0]);
        var f = (int[])[1];
        int[] g = [.. xs.Where(x => x > 0).Select(x => x * 2)];
    }

    int[] Init() => new[] { 1, 2 };
    int[,] Grid = new int[,] { { 1, 2 }, { 3, 4 } };
}
