//! HCL (Terraform, OpenTofu, Packer, Nomad, Consul): tree-sitter parses (through `core-treesitter`,
//! mapping in `mapping.rs`), `format.rs` lays the lines out like `terraform fmt`.

mod format;
mod mapping;
mod rules;

pub use format::{HclFormatter, option_schema};

include!("provider.generated.rs");
