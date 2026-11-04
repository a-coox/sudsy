class A {
 async Task A1() { }
 async Task<int> B1() { }
 async ValueTask C1() { }
 async System.Threading.Tasks.Task D1() { }
//                                 ^^ async-suffix
 async void E1() { }
 async Task OkAsync() { }
 public override async Task F1() { }
 async Task I.G1() { }
 async Task<int> Main() { }
 Task H1() { }
}
