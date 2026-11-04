using System.Diagnostics.CodeAnalysis;
[assembly: SuppressMessage("C", "I")]
[assembly: SuppressMessage("C", "I", Justification = "<Pending>")]
namespace N { class Other { void M() {
#region R
#endregion
} } class B { } struct S { } namespace M { } }
//        ^ single-class-per-file
