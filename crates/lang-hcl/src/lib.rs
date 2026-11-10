//! HCL (Terraform, OpenTofu, Packer, Nomad, Consul): tree-sitter parses (through `core-treesitter`,
//! mapping in `lang.yaml`), `format.rs` lays the lines out like `terraform fmt`.

mod format;
mod rules;

pub use format::{HclFormatter, option_schema};

include!("provider.generated.rs");
