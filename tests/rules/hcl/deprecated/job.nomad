job "web" {
  constraint {
    attribute = "${attr.kernel.name}"
    value     = "${NOMAD_PORT_http}"
  }
  id = aws_instance.web.0.id
}
