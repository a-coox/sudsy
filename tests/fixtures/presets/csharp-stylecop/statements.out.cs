using System;

namespace Parity.LayoutStatements
{
    public class Statements
    {
        private int a;

        public void Loops(int[] items)
        {
            for (; ;)
            {
                break;
            }

            for (int i = 0; ;)
            {
                break;
            }

            for (; a < 3;)
            {
                a++;
            }

            for (int i = 0; ;)
            {
                break;
            }

            foreach (var item in items)
            {
            }

            while (a < 3)
            {
            }

            lock (this)
            {
            }

            if (a == 1)
            {
            }
            else
            {
            }

            try
            {
                a++;
            }
            catch
            {
            }
            finally
            {
            }

            {
            }

            {
                a++;
            }

            Action<int> d = delegate(int v) { };
            Action<int> d2 = delegate(int v) { };
            Action e = () => { };
            void Local()
            {
            }
        }

        public void Blocks()
        {
            int z = 1;

            // Comment followed by a blank line.
            z++;
            if (z == 1)
            {
                z++;
            }

            z++;
            switch (z)
            {
                case 1:
                    {
                        z++;
                        break;
                    }

                case 2:
                    if (z == 1)
                    {
                        z++;
                    }

                    break;
                default:
                    break;
            }

            void Local()
            {
            }

            z++;
            do
            {
                z++;
            }
            while (z < 3);

            // Comment after a block.
            z++;

            // Standalone comment.

            // Second comment.
            z++;
            //// Commented out code is exempt.

            z++;
            if (z == 1)
            {
                z++;
            }
            else
            {
                z++;
            }

            while (z < 5)
            {
                z++;
            }
        }
    }
}
