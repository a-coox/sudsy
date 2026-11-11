switch (a) { case 1: break; case 1: break; case "x": break; case 'x': break; case b + 1: break; case b+1: break; default: }
//                          ^^^^^^^^^^^^^^ no-duplicate-case
//                                                                                              ^^^^^^^^^^^^^^^^ no-duplicate-case
if (a) {} else if (b) {} else if (a) {}
//                                ^ no-dupe-else-if
if (a || b) {} else if (a) {} else if (b && c) {}
//                      ^ no-dupe-else-if
//                                     ^^^^^^ no-dupe-else-if
if (a && b) {} else if (a) {} else if (b && a) {}
//                                     ^^^^^^ no-dupe-else-if
if (a) { if (a) {} } else if (c) {} else { if (c) {} }
