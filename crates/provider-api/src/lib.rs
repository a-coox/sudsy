//! The language provider interface and the out-of-process proxy that implements it for languages
//! parsed by a worker process (C#).

pub mod csharp;
pub mod out_of_process;
pub mod protocol;
pub mod provider;
pub mod proxy;

pub use out_of_process::OutOfProcessProvider;
pub use provider::{
    Capabilities, FileMatcher, FilePattern, LanguageProvider, ParseError, ParseResult,
    ProviderError, SourceFile,
};
pub use proxy::{ProxyError, WorkerConfig, WorkerProxy};
