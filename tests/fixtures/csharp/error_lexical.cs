class A
{
    string s = "unterminated;
    char c = 'ab';
    int n = 0x;
    // fine
    string t = $"{ open";
    /* never closed
