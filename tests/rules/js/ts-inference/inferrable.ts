const a: number = 5, b: string = `x`, c: boolean = !x, d: bigint = -1n, e: null = null, f: undefined = void 0;
//    ^^^^^^^^^^^^^ no-inferrable-types
//                   ^^^^^^^^^^^^^^^ no-inferrable-types
//                                    ^^^^^^^^^^^^^^^ no-inferrable-types
//                                                     ^^^^^^^^^^^^^^^ no-inferrable-types
//                                                                      ^^^^^^^^^^^^^^ no-inferrable-types
//                                                                                      ^^^^^^^^^^^^^^^^^^^^^ no-inferrable-types
const g: number = -Infinity, h: RegExp = /x/, i: symbol = Symbol("s"), j: number = x, k: string[] = [];
//    ^^^^^^^^^^^^^^^^^^^^^ no-inferrable-types
//                           ^^^^^^^^^^^^^^^ no-inferrable-types
//                                            ^^^^^^^^^^^^^^^^^^^^^^^ no-inferrable-types
function fn(l: number = 1, m = 2, n: string = x) {}
//          ^^^^^^^^^^^^^ no-inferrable-types
class C { p: number = 1; readonly q: number = 1; r?: number = 1; constructor(private s: string = "a") {} }
//        ^^^^^^^^^^^^^^ no-inferrable-types
//                                                                                   ^^^^^^^^^^^^^^^ no-inferrable-types
