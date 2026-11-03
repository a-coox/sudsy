namespace Parity
{
    public class Comments
    {
        //no space
        // one space
        //   several spaces
        //
        ////four slashes
        //-- dashes
        public int a; //trailing no space
        public int b; // trailing fine
        ///<summary>doc no space</summary>
        public int c;
        ///   <summary>doc with several spaces</summary>
        public int d;
        /// <summary>doc fine</summary>
        public int e;
        //
        // text after an empty comment
        /**/
        /* */
        /* text */
        //
        public void M()
        {
            //code();
            // fine
        }

        // Punctuation right after the slashes.
        //!bang
        //*star
        //#hash
        //@at
        //1digit
        //:colon
        //_under
        //====
        //***
        //-
        //----- x
        //// four and text
        ///// five slashes
        //	tab then text
        //x

        // Empty comments inside a block, first, last and alone.
        //
        // after first
        // before last
        //

        //

        // before middle
        //
        // after middle
        //
        //
        // after two empties

        public int f;
    }

    public class Probes
    {
        /* a */
        //
        // b
        public int m1;

        // a
        //
        /* b */
        public int m2;

        public int m3; //
        // after a trailing empty comment
        public int m4;

        /// doc
        //
        // after doc
        public int m5;

        //--
        //-x
        //--x
        //---
        //-x-
        //- x
        //=
        public int m6;

        /// <summary>
        /// Doc one.
        ///
        ///   deeper
        ///     deeper still
        ///	tab
        /// <remarks>
        /// text
        /// </remarks>
        /// </summary>
        public int m7;

        /// <summary>
        ///doc without space
        /// </summary>
        public int m8;

        /**
         * <summary>block doc</summary>
         */
        public int m9;

        ///
        public int m10;

        /// <summary>doc</summary>
        /// <value>text</value>
        public int m11;

        // p
        //
#if ALPHA
        public int m12;
#endif

        /// <summary>
        ///   <b>element first</b> text
        /// </summary>
        public int p1;

        /// <summary>text
        ///   more text
        /// </summary>
        public int p2;

        ///   text only on the first line
        public int p3;

        /// <summary>
        ///     <c>x</c>
        ///     y
        /// </summary>
        public int p4;

        /// <summary>
        ///  two spaces
        /// </summary>
        public int p5;

        /// <summary>
        /// <code>
        ///   indented code
        /// </code>
        /// </summary>
        public int p6;

        /// <summary>
        /// <list type="bullet">
        ///   <item>one</item>
        /// </list>
        /// </summary>
        public int p7;
    }
}
