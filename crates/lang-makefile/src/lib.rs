mod embedded;
mod format;
mod rules;

pub use format::{MakefileFormatter, option_schema};

include!("provider.generated.rs");
