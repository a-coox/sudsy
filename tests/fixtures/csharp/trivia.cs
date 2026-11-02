// leading file comment

/* block */ using System; // trailing after using
using System.Linq; /* trailing block */ // and a line comment

/* before class */
class C /* after name */ : Base /* after base */
{ // after open brace
	int tabs; // tab indented

    int a, // after comma
        b;

    /* multi
       line */ int c;

    void M(/* in params */ int x /* after x */, int y)
    {
        var s = x /* mid */ + /* mid2 */ y; // end


        // a comment alone on a line
        return;
    } // after close brace
} // after class
// last line comment, no newline at end