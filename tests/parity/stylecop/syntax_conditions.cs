namespace Fx.SyntaxConditions
{
    public class Conditions
    {
        public int Field;

        public void Method(object x, int i, string s)
        {
            bool b1 = null == x;
            bool b2 = 1 == i;
            bool b3 = "a" == s;
            bool b4 = true == (i > 0);
            bool b5 = int.MaxValue == i;
            bool b6 = -1 == i;
            bool b7 = 1 == 2;
            bool b8 = 1 < i;
            bool b9 = 1 <= i;
            bool b10 = 1 > i;
            bool b11 = 1 >= i;
            bool b12 = 1 != i;
            bool b13 = default == x;
            bool b14 = (1) == i;
            bool b15 = 1 + 2 == i;
            bool b16 = 'c' == s[0];
            bool b17 = null != x;
            bool b18 = null == null;
            bool b19 = 1 == i + 1;
            bool b20 = 1 == Field;
            bool b21 = 2.5 == (double)i;
            bool b22 = typeof(int) == x.GetType();
            bool b23 = 1 < i && 2 < i;
            bool b24 = Field == 1;
            bool b25 = nameof(i) == s;
            bool b26 = 1 + i == 2;
            bool b27 = null == s?.Trim();
            bool b28 = 1 /* c */ == i;
        }
    }
}
