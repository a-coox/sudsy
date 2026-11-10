resource "a" "b" {
  name = var.name
  tags = [var.a, "x-${var.b}", "${var.c}${var.d}"]
  id   = var.e
  raw  = "${~ var.f ~}"
  id2  = aws_instance.web[0].id
  all  = aws_instance.web[0].id
  ids  = aws_instance.web.*.id
  num  = local.versions[1][2]
  key  = { (var.k) = 1 }
  sum  = 1 + "${var.n}"
}
