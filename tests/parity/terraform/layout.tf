x1 = {a="x",b = "y" ,c:3}
y = [ ]
z = {   }
w = a[ 0 ]
v = a [0]
u = a-1
t = a - -1
r = a *b
q = (a)
p = ( a + b )
o = "${a}${ b }"
n = "%{ if x }y%{ else }z%{ endif }"
l = foo(
  a,
  b
)
k = [
  1,
2,
    3,
]
j = <<-EOT
    hi
      ${ a   }
    EOT
i = {
  a = 1   # one
  bbbb = 2 # two
  c = 3
  # standalone
  dd = 4   // four
}
h = (a ?
  b :
  c)
g=!a
f=a&&b||c
e = a.b . c
d = "é${x}"
c = "a" # trail
bb = [for x in y: x]
aaa = {for x in y: x => z...}
resource "x" "y" {
  a = 1
  nested {
    b   = 2
  }
  cc = 3
  d = {
    e = 1
  }
  ff = 4
}
tt	= 1
	tab = 2
/* multi
   line */
ee = 1 /* after */ + 2
x2 = a.0.b
x4 = var.map["a"]
x5 = a[*].b
x7 = a ==b
x8 = a>=b
x9 = a%b
x11 = [1,2,3,]
x12 = foo(a...)
x13 = a ? [1] : {}
x14 =<<EOT
a
EOT
yy=1
