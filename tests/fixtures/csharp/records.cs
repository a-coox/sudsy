namespace Records
{
    public record Person(string First, string Last) : Base(First)
    {
        public string Full => $"{First} {Last}";
    }

    public readonly record struct Point(int X, int Y);

    public record class Empty;

    public record Derived(int Id, [property: Obsolete] string Name = "x") : Person("a", "b");

    public class Service(IRepository repo, ILogger log) : ServiceBase(repo)
    {
        private readonly ILogger _log = log;
    }

    public struct Pair<T>(T a, T b)
    {
        public T A { get; init; } = a;
    }

    public static class Use
    {
        public static Person Rename(Person p) => p with { First = "Z", Last = "Q" };
    }
}
