a = 1
a = 2
^ no-duplicate-keys
block {
  x = 1
  x = 2
# ^ no-duplicate-keys
  y = { k = 1, k = 2 }
#              ^ no-duplicate-keys
}
block {
  x = 1
}
