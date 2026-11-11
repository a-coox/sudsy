let a: "a" = "a", b: "a" = 'a', c: 1 = 1, d: string = "a";
//     ^^^ prefer-as-const
//                                 ^ prefer-as-const
//                                        ^^^^^^^^^^^^^^^ no-inferrable-types
class C { p: "a" = "a"; q = 1 as 1; r = <"a">"a"; s = "a" as "b"; }
//           ^^^ prefer-as-const
//                               ^ prefer-as-const
//                                       ^^^ prefer-as-const
