var s = "\d \a \' \" \\ \n \x41 A \0 \8 \
//       ^^ no-useless-escape
//          ^^ no-useless-escape
//             ^^ no-useless-escape
 ok";
var t = '\' \" \d';
//          ^^ no-useless-escape
//             ^^ no-useless-escape
var u = `\d \` \$ \${a} \{ $\{ ${b}\$`;
//       ^^ no-useless-escape
//             ^^ no-useless-escape
//                      ^^ no-useless-escape
//                                 ^^ no-useless-escape
var tag = x`\d`;
var r = /\d \- \/ [\-] [a\-] [\^] [^\^] [\/] \. \B \k \a/;
//          ^^ no-useless-escape
//                 ^^ no-useless-escape
//                       ^^ no-useless-escape
//                                  ^^ no-useless-escape
//                                       ^^ no-useless-escape
//                                                    ^^ no-useless-escape
var n = new RegExp("\d");
//                  ^^ no-useless-escape
