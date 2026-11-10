variable "plain" {}
^^^^^^^^ documented-variables
^^^^^^^^ typed-variables

variable "full" {
  type        = string
  description = "Documented."
  validation {
    condition     = length(var.full) > 0
    error_message = "empty"
  }
}

output "bare" {
^^^^^^ documented-outputs
  value = 1
}

output "documented" {
  value       = 2
  description = "Documented."
}
