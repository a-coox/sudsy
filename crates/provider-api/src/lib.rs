//! The language provider interface and the out-of-process proxy that implements it for languages
//! parsed by a worker process (C#).

pub mod fixtures;
pub mod format;
pub mod out_of_process;
pub mod protocol;
pub mod provider;
pub mod proxy;
pub mod snippets;
pub mod testing;

pub use format::{
    FileFormatError, format_file, format_parsed, resolve_dialect, select_provider,
    select_provider_index,
};
pub use out_of_process::OutOfProcessProvider;
pub use provider::{
    Capabilities, FileMatcher, FilePattern, LanguageProvider, ParseError, ParseResult,
    ProviderError, RUNTIME_MISSING, RUNTIME_URL, SourceError, SourceFile,
};
pub use proxy::{ProxyError, WorkerConfig, WorkerProxy};
pub use snippets::{FnSnippets, ProviderSnippets, with_pattern_rules};
