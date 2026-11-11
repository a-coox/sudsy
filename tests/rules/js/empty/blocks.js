if (a) {} else { /* kept */ }
//     ^^ no-empty
try { f(); } catch (e) {} finally {}
//                     ^^ no-empty
//                                ^^ no-empty
try { f(); } catch (e) { throw e; }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-useless-catch
try { f(); } catch (e) { throw e; } finally { g(); }
//           ^^^^^^^^^^^^^^^^^^^^^^ no-useless-catch
try { f(); } catch (e) { throw g; }
switch (a) {}
//         ^ no-empty
function f() {}
var g = () => {};
for (;;) {}
//       ^^ no-empty
class K { static {} }
var {} = o, [] = p, [,] = q, { a: {} } = r;
//  ^^ no-empty-pattern
//          ^^ no-empty-pattern
//                                ^^ no-empty-pattern
({} = o);
 ^^ no-empty-pattern
var x = [1, , 2, ,], y = [[], [,]];
//          ^ no-sparse-arrays
//               ^ no-sparse-arrays
//                             ^ no-sparse-arrays
var r = /a[]/, s = /[]]/, t = /[^]/, u = /\[]/, v = /[\]]/, w = new RegExp("[]");
//      ^^^^^ no-empty-character-class
//                 ^^^^^ no-empty-character-class
