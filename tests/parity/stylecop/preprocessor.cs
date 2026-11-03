#define ALPHA
namespace Parity
{
    public class Preprocessor
    {
#if DEBUG
        public int a;
#endif
        # if DEBUG
        public int b;
        #endif
#region Named
        public int c;
#endregion
        #region Indented
        public int d;
        #endregion
        # region Spaced
        public int e;
        # endregion
#pragma warning disable CS0169
#  pragma warning restore CS0169
#if ALPHA
        public int f;
#  elif BETA
        public int g;
#   else
        public int h;
# endif
#if false
# if X
#region InDisabled
#endregion
# endif
#endif
#nullable enable
# nullable disable
    }
}
