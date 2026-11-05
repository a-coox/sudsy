using System.Diagnostics.CodeAnalysis;
[assembly: SuppressMessage("C", "I")]
//         ^^^^^^^^^^^^^^^ suppression-justification
[assembly: SuppressMessage("C", "I", Justification = "<Pending>")]
//                                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^ suppression-justification
namespace N { class Other { void M() {
#region R
^ no-region-in-body
#endregion
} } class B { } struct S { } namespace M { } }
//        ^ single-class-per-file
//                                     ^ single-namespace-per-file
