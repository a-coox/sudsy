using System;

namespace Parity
{
    public class Statements
    {
        public void Empty(bool x)
        {
            ;
            int y = 1;;
            if (x)
            {
                ;
            }
            while (x) ;
            for (;;) { break; }
            if (x);
            else;
            switch (y)
            {
                case 1:
                    ;
                    break;
            }
            label:
            ;
        }

        public void Comments(bool x)
        {
            if (x /* c1 */)
            {
            }

            if (x) /* c2 */
            {
            }
            else /* c3 */
            {
            }

            if (x) // c4
            {
            }

            while (x /* c5 */) { }
            for (int i = 0; /* c6 */ i < 1; i++) { }
            foreach (var z in new int[0]) /* c7 */ { }
            try /* c8 */ { } catch /* c9 */ { }
            if (x)
            {
                /* inside is fine */
            }
        }

        public void Blocks(bool x, object o)
        {
            do /* d1 */ { } while (x);
            using (o as IDisposable) /* u1 */ { }
            lock (o) /* l1 */ { }
            if (x)
                // own line before the brace
            {
            }
            /* bare */ { }
            try { } /* t1 */ finally /* f1 */ { }
            switch (o) /* s1 */ { }
            Action a = () /* a1 */ => { };
            if (x) { } /* after */ else { }
            if (x) { } else if (x) /* ei */ { }
            unchecked /* un */ { }
            if (x) { // after the open brace
            }
            if (x)
            {
            } // after the close brace
            if (x) /* e1 */ x = !x;
            if (x) { } else /* e2 */ if (x) { }
            try { } catch (Exception) /* c10 */ { }
            try { } catch (Exception) when (x) /* c11 */ { }
            checked /* ck */ { }
            while (x) /* w1 */ { }
            for (;;) /* f2 */ { break; }
            for (;;) // f3
            { break; }
            if (x)
                /* own line block comment */
            { }
            using (o as IDisposable) // u2
            { }
            if (x) { /* inside */ }
        }

        public void MethodBody() /* m1 */ { }

        public int Property { get /* g1 */ { return 1; } }

        public void EmbeddedEmpty(bool x, object o)
        {
            for (;;) ;
            do ; while (x);
            foreach (var z in new int[0]) ;
            lock (o) ;
            if (x) { } else if (x) ;
            { };
            { ; }
            Action a = () => { ; };
            switch (o)
            {
                case null: ; break;
                default: ;
            }
            label2: ;
        }

        public void Delegates()
        {
            Action a = delegate() { };
            Action b = delegate { };
            Action<int> c = delegate(int v) { };
            Func<int> d = delegate () { return 1; };
            Action e = delegate () { };
            Action f = delegate( ) { };
        }
    }
}
