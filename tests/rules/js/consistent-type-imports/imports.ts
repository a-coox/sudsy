import { A, B } from "a";
^^^^^^^^^^^^^^^^^^^^^^^^^ consistent-type-imports
import C, { D } from "c";
^^^^^^^^^^^^^^^^^^^^^^^^^ consistent-type-imports
import * as NS from "ns";
^^^^^^^^^^^^^^^^^^^^^^^^^ consistent-type-imports
import E from "e";
^^^^^^^^^^^^^^^^^^ consistent-type-imports
import type { T } from "t";
import { type U, V } from "u";
import { W, Y } from "w";
^^^^^^^^^^^^^^^^^^^^^^^^^ consistent-type-imports
import { X } from "x";
import Z from "z";
export { X };
let a: A, b: B, c: C, d: D, ns: NS.Thing, w: W = 1, q: typeof E, t: T, u: U, y: Y;
const v = V, z = Z;
type I = import("m").I;
//       ^^^^^^^^^^^^^ consistent-type-imports
