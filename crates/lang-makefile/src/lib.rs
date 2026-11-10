core_syntax::include_generated!();
mod embedded;
mod format;
mod mapping;
mod rules;

pub use format::{MakefileFormatter, option_schema};

include!("provider.generated.rs");
