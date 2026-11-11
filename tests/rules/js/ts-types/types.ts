let a: any = 1;
//     ^^^ no-explicit-any
type A = {};
//   ^ consistent-type-definitions
//       ^^ no-empty-object-type
type B = {} & { a: 1 };
interface C {}
//        ^ no-empty-object-type
interface D extends A {}
//        ^ no-empty-object-type
interface E extends A, B {}
interface F { a: 1 }
type L = { a: 1 };
//   ^ consistent-type-definitions
type M = A | B;
let g: Array<number>, h: ReadonlyArray<string>, i: Readonly<string[]>, j: Array<number, string>, k: number[];
//     ^^^^^^^^^^^^^ array-type
//                       ^^^^^^^^^^^^^^^^^^^^^ array-type
//                                                 ^^^^^^^^^^^^^^^^^^ array-type
