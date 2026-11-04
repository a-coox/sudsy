using System.Threading.Tasks;
interface I { Task Run(); }
class A : I {
 public async Task Run() { }
 public async Task Other() { }
//                 ^^^^^ async-suffix
}
class B {
 class Task { }
 async Task Mine() { }
}
