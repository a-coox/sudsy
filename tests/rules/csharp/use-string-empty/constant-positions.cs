class A {
 const string C = "";
 string F(string s = "") {
  if (s is "") { }
  switch (s) { case "": break; }
  return s ?? @"";
//            ^^^ use-string-empty
 }
 [System.Obsolete("")] void G() { }
}
