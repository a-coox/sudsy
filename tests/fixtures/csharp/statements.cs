class Statements
{
    unsafe void M(int[] xs)
    {
        for (int i = 0, j = 10; i < j; i++, j--) { continue; }
        for (;;) break;
        for (i = 0; ; ) { }
        foreach (var (a, b) in pairs) { }
        foreach (ref readonly var r in refs) { }
        await foreach (var x in stream) { }
        while (true) { if (done) break; }
        do { x++; } while (x < 10);
        using var file = Open();
        using (var a = A()) using (b) { }
        await using (var c = C()) { }
        lock (this) { }
        fixed (int* p = xs) { }
        checked { x++; }
        unchecked { x++; }
        unsafe { }
        try { Run(); }
        catch (ArgumentException e) when (e.Message != null) { throw; }
        catch (Exception) { }
        catch { throw new Exception("x"); }
        finally { Cleanup(); }
        switch (x)
        {
            case 1: goto case 2;
            case 2: goto default;
            default: break;
        }
        label: x++;
        goto label;
        var (p1, p2) = (1, 2);
        const int K = 3;
        int[] arr = new int[3];
        ;
        yield return 1;
        yield break;
        if (x) y(); else if (z) w(); else { }
        return;
    }
}
