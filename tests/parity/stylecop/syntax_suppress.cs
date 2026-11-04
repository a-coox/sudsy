using System.Diagnostics.CodeAnalysis;

[assembly: SuppressMessage("C", "I")]
[assembly: SuppressMessage("C", "I", Justification = "")]
[assembly: SuppressMessage("C", "I", Justification = " ")]
[assembly: SuppressMessage("C", "I", Justification = "<Pending>")]
[assembly: SuppressMessage("C", "I", Justification = "ok")]
[assembly: SuppressMessage("C", "I", Justification = null)]
[assembly: SuppressMessage("C", "I", Justification = "a" + "b")]
[assembly: SuppressMessageAttribute("C", "I")]
[assembly: System.Diagnostics.CodeAnalysis.SuppressMessage("C", "I")]
[assembly: UnconditionalSuppressMessage("C", "I")]

namespace Fx.SyntaxSuppress
{
    public class Suppress
    {
        [SuppressMessage("C", "I")]
        public void M() { }

        [SuppressMessage("C", "I", MessageId = "m")]
        public void N() { }

        [SuppressMessage("C", "I", Justification = "x"), SuppressMessage("C", "J")]
        public void O() { }
    }
}
