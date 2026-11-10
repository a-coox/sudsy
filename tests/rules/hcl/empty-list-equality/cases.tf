a = var.l == []
#   ^^^^^^^^^^^ empty-list-equality
b = var.l != []
#   ^^^^^^^^^^^ empty-list-equality
c = [] == var.l
#   ^^^^^^^^^ empty-list-equality
d = length(var.l) == 0
e = var.l == [] && var.m
#   ^^^^^^^^^^^ empty-list-equality
f = var.l == [1]
