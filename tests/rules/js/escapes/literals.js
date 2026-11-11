var a = /a  b/, b = /a {2}b/, c = /[  ]/, d = /a  +/, e = /a   +/, f = new RegExp("a  b"), g = RegExp("a b"), h = /a\  b/;
//      ^^^^^^ no-regex-spaces
//                                                        ^^^^^^^ no-regex-spaces
//                                                                     ^^^^^^^^^^^^^^^^^^ no-regex-spaces
//                                                                                                                ^^^^^^^ no-regex-spaces
//                                                                                                                  ^^ no-useless-escape
var i = 017, j = 08, k = 0, l = 0.5, m = 0o17;
//      ^^^ no-octal
//               ^^ no-octal
var p = 9007199254740993, q = 0.1, r = 5123000000000000000000000000001, s = 0x20000000000001, t = 0x1fffffffffffff;
//      ^^^^^^^^^^^^^^^^ no-loss-of-precision
//                                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-loss-of-precision
//                                                                          ^^^^^^^^^^^^^^^^ no-loss-of-precision
var u = 12345678901234567890, v = 0.30000000000000004, w = 1_000, x = 0.1000000000000000055511151231257827, y = 1e3;
//      ^^^^^^^^^^^^^^^^^^^^ no-loss-of-precision
