// A header comment, kept apart from the code below.

using System;
using System.IO;
namespace Parity.LayoutMembers
{
    public class Empty { }
    public enum Colors { }
    public enum Named
    {
        /// <summary>First.</summary>
        First,
        /// <summary>Second.</summary>
        Second,
        Third,
        // A plain comment.
        Fourth,
    }
    public class Members
    {
        private int a;
        private int b;
        [Obsolete]
        private int c;
        public event Action E;
        public event Action F;
        public int P { get; set; }
        public int Q { get; set; }
        /// <summary>Documented.</summary>

        public int R { get { return a; } }
        public Members() { }
        public void M() { }
        // Plain comment.
        public void N() { }
        /// <summary>Documented.</summary>
        public void O() { }
        public void Wide(
            int first,
            int second
        )
        {
            Wide(
                1,
                2
            );
        }
        [Obsolete(
            "message"
        )]
        public void Attributed() { }
    }
    public struct S
    {
    }
}
