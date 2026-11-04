class A { void F() {
 // sudsy-disable-next-line no-empty-catch
 try { } catch { }
 try { } catch { }
//       ^^^^^ no-empty-catch
} }
