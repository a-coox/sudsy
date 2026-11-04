using System;
using System.Collections.Generic;

namespace N {
    class C {
        int P { get; set; }
        int Q {
            get {
                return 1;
            }
            set {
                value++;
            }
        }
        void M(int x) {
            if (x > 1) {
                x++;
            }
            else {
                x--;
            }
            while (x > 0) {
                x--;
            }
            try {
                x++;
            }
            catch (Exception) {
                x--;
            }
            finally {
                x = 0;
            }
        lbl:
            x++;
            goto lbl;
            Action a = () => {
                x++;
            };
            Func<int, int> f = delegate (int y) {
                return y;
            };
            switch (x) {
                case 1:
                    break;
            }
            int Local() {
                return 1;
            }
        }
    }
}
