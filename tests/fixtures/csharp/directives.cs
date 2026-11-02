#define FEATURE
#undef OLD
#nullable enable
#pragma warning disable CS0168, CS0219 // unused
using System;

namespace Directives
{
    #region Members
    class C
    {
#if FEATURE
        void Active() { }
#elif LEGACY && !FEATURE
        void Legacy() { int never = 1; }
#else
        void Fallback() { not even valid C# $$$ }
#endif

#if DEBUG || (NET8_0_OR_GREATER && !TRACE)
        void Debug() { }
#endif
        int x; // after
#pragma warning restore CS0168
#pragma checksum "file.cs" "{406EA660-64CF-4C82-B6F0-42D48172A799}" "ab007f1d23d9"
#line 200 "generated.cs"
        int y;
#line hidden
#line default
#nullable restore
#warning be careful
    }
    #endregion
}
