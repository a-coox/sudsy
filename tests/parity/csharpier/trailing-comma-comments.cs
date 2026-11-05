class C {
    int[] a = new[] {
        1,
        2 // two
    };
    int[] b = new[] {
        1,
        2, // two
    };
    enum E { A, B // b
    }
    X Y {
        get { return new TheoryData<string, StringSegment[]> {
            { "a", new StringSegment[] { new StringSegment("aaaaaaaaaaaaaaaaaaaa", 1, 0), new StringSegment("aaaaaaaaaaaaaaaaaaaa", 2, 0) } },
            { "b", new StringSegment[] { } },
        }; }
    }
}
