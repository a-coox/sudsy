resource "a" "b" {
  name = "${var.name}"
#        ^^^^^^^^^^^^^ deprecated-interpolation
  tags = ["${var.a}", "x-${var.b}", "${var.c}${var.d}"]
#         ^^^^^^^^^^ deprecated-interpolation
  id   = "${ var.e }"
#        ^^^^^^^^^^^^ deprecated-interpolation
  raw  = "${~ var.f ~}"
  id2  = aws_instance.web.0.id
#                        ^^ deprecated-index
  all  = aws_instance.web[0].id
  ids  = aws_instance.web.*.id
  num  = local.versions.1.2
#                      ^^ deprecated-index
#                        ^^ deprecated-index
  key  = { "${var.k}" = 1 }
#          ^^^^^^^^^^ deprecated-interpolation
  sum  = 1 + "${var.n}"
#            ^^^^^^^^^^ deprecated-interpolation
}
