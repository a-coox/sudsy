variable "good_name" {}
variable "badName" {}
#        ^^^^^^^^^ naming-convention
variable "also-bad" {}
#        ^^^^^^^^^^ naming-convention
resource "aws_instance" "web_server" {}
resource "aws_instance" "WebServer" {}
#                       ^^^^^^^^^^^ naming-convention
data "aws_ami" "Latest" {}
#              ^^^^^^^^ naming-convention
module "Network" {}
#      ^^^^^^^^^ naming-convention
output "out_1" {}
locals {
  fine_value = 1
  BadValue   = 2
# ^^^^^^^^ naming-convention
}
