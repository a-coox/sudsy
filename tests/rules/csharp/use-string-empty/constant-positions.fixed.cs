class A {
 const string C = "";
 string F(string s = "") {
  if (s is "") { }
  switch (s) { case "": break; }
  return s ?? string.Empty;
 }
 [System.Obsolete("")] void G() { }
}
