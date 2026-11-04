using System;

namespace N
{
    class C
    {
        int P { get { return 1; } set { value++; } }
        int Q
        {
            get { return 1; }
        }
        int R() { return 1; }
        void M2() { M3(); M4(); }
        void M(int x)
        {
            if (x > 1) { x++; } else { x--; }
            while (x > 0) { x--; }
            int a = 1; int b = 2;
            x++; x--;
            try { x++; } catch (Exception) { x--; } finally { x = 0; }
            if (x > 1) { x++; x--; }
            for (int i = 0; i < 1; i++) { x++; }
            foreach (var q in new int[0]) { a++; b++; }
            if (x > 1) { if (a > 1) { b++; } }
            switch (x)
            {
                case 1: break;
                case 2:
                    x++; x--;
                    break;
            }
        }
    }
}
