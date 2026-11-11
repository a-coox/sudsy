var a = 1;
//     ^ no-irregular-whitespace
var b = " ";
// a comment with a space
//               ^ no-irregular-whitespace
var c = ` `;
//       ^ no-irregular-whitespace
var d = / /;
//       ^ no-irregular-whitespace
var e =  f;
//     ^^ no-irregular-whitespace
var g = "\u00a0";
