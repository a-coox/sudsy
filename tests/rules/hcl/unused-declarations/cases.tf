variable "used" {}
variable "unused" {}
#        ^^^^^^^^ unused-declarations
locals {
  used_local   = var.used
  unused_local = 1
# ^^^^^^^^^^^^ unused-declarations
}
data "aws_ami" "used" {}
data "aws_ami" "unused" {}
#              ^^^^^^^^ unused-declarations
output "x" {
  value = "${local.used_local}-${data.aws_ami.used.id}"
}
