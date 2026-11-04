using System;

namespace Fx.SyntaxLambda
{
    public class Lambdas
    {
        public void Method()
        {
            Action a1 = delegate { };
            Action a2 = delegate() { };
            Action<int> a3 = delegate(int x) { };
            Func<int, int> a4 = delegate(int x) { return x; };
            Func<int, Func<int>> a5 = delegate (int x) { return delegate { return x; }; };
            Action<int, int> a6 = delegate(int x, int y) { };
            Action<int> a7 = delegate(int x)
            {
            };
            Action<int> a8 = delegate /* c */ (int x) { };
            Action<int> a9 = delegate(int x) { }
            ;
            Action<int> ok = x => { };
        }
    }
}
