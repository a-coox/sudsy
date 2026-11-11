namespace N.M {}
^^^^^^^^^^^^^^^^ no-namespace
declare module "x" {}
declare global {}
declare namespace Q {}
^^^^^^^^^^^^^^^^^^^^^^ no-namespace
const a = require("a"), b = a.require("b");
//        ^^^^^^^^^^^^ no-require-imports
import c = require("c");
//         ^^^^^^^^^^^^ no-require-imports
const self = this, { x } = this;
//    ^^^^ no-this-alias
y = this;
^ no-this-alias
class K { new(): K; new(): number; new() {} }
//        ^^^^^^^^^ no-misused-new
interface I { new (): I; new (): number; constructor(): void }
//            ^^^^^^^^^^ no-misused-new
//                                       ^^^^^^^^^^^^^^^^^^^ no-misused-new
function f(): void;
function g(): void {}
function f(x: number): void;
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ adjacent-overload-signatures
function f(x?: number) {}
class O { m(): void; n(): void; m(x: number): void; static m(): void; static m(x: number): void; }
//                              ^^^^^^^^^^^^^^^^^^^ adjacent-overload-signatures
interface P { (): void; a(): void; (x: number): void }
//                                 ^^^^^^^^^^^^^^^^^ adjacent-overload-signatures
