


using System;

namespace Parity.LayoutBlank
{

    public class Blank
    {

        private int a;


        private int b;

        public int Mixed { get { return a; } set
            {
                a = value;
            } }

        public int Spread
        {
            get { return b; }

            set { b = value; }
        }

        public void Statements()
        {

            if (a == 1)
            {
                a++;

            }

            else
            {
                a--;
            }

            do
            {
                a++;
            }

            while (a < 3);


            a++;
        }

    }
}
