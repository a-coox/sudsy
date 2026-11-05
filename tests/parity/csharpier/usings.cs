using Zed;
using global::Foo;
using System.Linq;
using System;
using global::System.Text;
#if A
using Z2;
using B2;
#endif
#if B
using Q;
#else
using P;
#endif
using Beta;
// kept with Alpha
using Alpha;
global using Y;
global using static System.Linq.Enumerable;
global using X;

class C { }
