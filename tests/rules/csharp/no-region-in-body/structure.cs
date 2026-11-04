using System.Diagnostics.CodeAnalysis;
[assembly: SuppressMessage("C", "I")]
[assembly: SuppressMessage("C", "I", Justification = "<Pending>")]
namespace N { class Other { void M() {
#region R
^ no-region-in-body
#endregion
} } class B { } struct S { } namespace M { } }
