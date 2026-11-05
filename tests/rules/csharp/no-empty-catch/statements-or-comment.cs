class A { void F() {
 try { } catch (Exception) { }
//       ^^^^^ no-empty-catch
 try { } catch { /* ignored on purpose */ }
 try { } catch { Log(); }
 try { } catch { try { } catch { Log(); } }
} }
