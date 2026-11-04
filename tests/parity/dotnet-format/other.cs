using System;
using System.Collections.Generic;

namespace N {
    class C {
        int[] items = new int[2];
        int this[int i] {
            get {
                return items[i];
            }
            set {
                items[i] = value;
            }
        }
        event EventHandler E {
            add {
                Console.WriteLine();
            }
            remove {
                Console.WriteLine();
            }
        }
        void M() {
            Func<int> a = delegate {
                return 1;
            };
            Action b = () => {
                Console.WriteLine();
            };
            var list = new List<int> {
                1,
                2
            };
            var anon = new {
                A = 1,
                B = 2
            };
        }
    }
}
