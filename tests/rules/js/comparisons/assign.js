a = a; a.b = a.b; a["b"] = a.b; a[b] = a[b]; this.c = this.c; a = b; a.b = a.c; a &&= a; a += a;
//  ^ no-self-assign
//           ^^^ no-self-assign
//                         ^^^ no-self-assign
//                                     ^^^^ no-self-assign
//                                                    ^^^^^^ no-self-assign
//                                                                                    ^ no-self-assign
[a, b] = [a, b]; [a, ...b] = [a, ...b]; [a, b] = [b, a]; ({ a } = { a }); ({ a: b } = { a: b });
//        ^ no-self-assign
//           ^ no-self-assign
//                            ^ no-self-assign
//                                  ^ no-self-assign
//                                                                  ^ no-self-assign
//                                                                                         ^ no-self-assign
if (a = b) {} if ((a = b)) {} while (a = b) {} do {} while (a = b); for (; a = b; ) {} for (; (a = b); ) {}
//  ^^^^^ no-cond-assign
//                                   ^^^^^ no-cond-assign
//                                                          ^^^^^ no-cond-assign
//                                                                         ^^^^^ no-cond-assign
var x = (a = b) ? 1 : 2, y = a = b ? 1 : 2;
//       ^^^^^ no-cond-assign
if (!!a) {} while (!!a) {} do {} while (!!a); for (; !!a; ) {} var z = !!a ? 1 : 2, w = !!a;
//  ^^^ no-extra-boolean-cast
//                 ^^^ no-extra-boolean-cast
//                                      ^^^ no-extra-boolean-cast
//                                                   ^^^ no-extra-boolean-cast
//                                                                     ^^^ no-extra-boolean-cast
if (Boolean(a)) {} if (!Boolean(a)) {} Boolean(!!a); !!!a; if ((!!a)) {} if (a && !!b) {} var u = Boolean(a);
//  ^^^^^^^^^^ no-extra-boolean-cast
//                      ^^^^^^^^^^ no-extra-boolean-cast
//                                             ^^^ no-extra-boolean-cast
//                                                    ^^^ no-extra-boolean-cast
//                                                              ^^^ no-extra-boolean-cast
