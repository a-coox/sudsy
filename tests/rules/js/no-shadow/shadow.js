var a = 1; let b = 2;
function f(a) { let b = 3; { const a = 4; } }
//         ^ no-shadow
//                  ^ no-shadow
//                                 ^ no-shadow
var c = function c() {}; var d = function (d) {};
//                                         ^ no-shadow
function outer() { var e = 1; function inner() { var e = 2; } }
//                                                   ^ no-shadow
function hoist() { var h = 1; } function h() {}
//                     ^ no-shadow
const t = () => { const late = 1; }; const late = 2;
class K { m() { var K = 1; } }
//                  ^ no-shadow
