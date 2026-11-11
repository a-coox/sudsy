var o = { a: 1, a: 2, "b": 1, b: 2, 1: 1, 1.0: 2, [c]: 1, [c]: 2, ["d"]: 1, d: 2 };
//              ^ no-dupe-keys
//                            ^ no-dupe-keys
//                                        ^^^ no-dupe-keys
//                                                                          ^ no-dupe-keys
var p = { get g() { return 1; }, set g(v) {}, h: 1, get h() { return 1; }, ...s, ...s };
//                                                      ^ no-dupe-keys
var q = { a, a, m() {}, m() {} };
//           ^ no-dupe-keys
//                      ^ no-dupe-keys
class A { a() {} a() {} static b() {} b() {} get c() { return 1; } set c(v) {} d = 1; d = 2; ["e"]() {} e() {} }
//               ^^^^^^ no-dupe-class-members
//                                                                                    ^^^^^^ no-dupe-class-members
//                                                                                                      ^^^^^^ no-dupe-class-members
