terraform {
^^^^^^^^^ required-providers
^^^^^^^^^ required-version
  backend "s3" {}
}

terraform {
  required_version = ">= 1.5"
  required_providers {
    aws = { source = "hashicorp/aws" }
  }
}
