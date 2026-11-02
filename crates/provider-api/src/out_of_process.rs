use crate::protocol::ParseRequest;
use crate::provider::{
    Capabilities, FileMatcher, LanguageProvider, ParseError, ParseResult, ProviderError, SourceFile,
};
use crate::proxy::{ProxyError, WorkerConfig, WorkerProxy};
use core_config::OptionSchema;
use core_format::{DefaultFormatter, NodeFormatter};
use core_syntax::{Grammar, LanguageId, SyntaxNode};

/// A provider whose parser runs in a worker process.
pub struct OutOfProcessProvider {
    id: LanguageId,
    grammar: &'static Grammar,
    matchers: Vec<FileMatcher>,
    proxy: WorkerProxy,
    /// No format options until the C# formatter exists (M2b).
    schema: OptionSchema,
}

impl OutOfProcessProvider {
    /// The worker is started lazily, on the first call that needs it.
    pub fn new(
        id: LanguageId,
        grammar: &'static Grammar,
        matchers: Vec<FileMatcher>,
        worker: WorkerConfig,
    ) -> Self {
        OutOfProcessProvider {
            id,
            grammar,
            matchers,
            proxy: WorkerProxy::new(worker),
            schema: OptionSchema::default(),
        }
    }

    pub fn proxy(&self) -> &WorkerProxy {
        &self.proxy
    }
}

fn to_provider_error(e: ProxyError) -> ProviderError {
    match e {
        ProxyError::Unavailable(m) => ProviderError::Unavailable(m),
        ProxyError::Worker(p) => ProviderError::Worker {
            code: p.code,
            message: p.message,
        },
    }
}

impl LanguageProvider for OutOfProcessProvider {
    fn id(&self) -> LanguageId {
        self.id
    }

    fn grammar(&self) -> &Grammar {
        self.grammar
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        self.proxy.capabilities().unwrap_or(Capabilities::NONE)
    }

    fn formatter(&self) -> &dyn NodeFormatter {
        &DefaultFormatter
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        let parsed = self
            .proxy
            .parse(ParseRequest {
                file_id: file.id.clone(),
                version: file.version,
                text: file.text.clone(),
                language_version: file.language_version.clone(),
                preprocessor_symbols: file.preprocessor_symbols.clone(),
            })
            .map_err(to_provider_error)?;
        if parsed.file_id != file.id || parsed.version != file.version {
            return Err(ProviderError::Unavailable(
                "worker answered for a different file or version".into(),
            ));
        }
        let tree = SyntaxNode::new_root(parsed.tree.decode(&file.text)?);
        let errors = parsed
            .errors
            .into_iter()
            .map(|e| ParseError {
                start: e.start,
                len: e.length,
                code: e.code,
                message: e.message,
            })
            .collect();
        Ok(ParseResult { tree, errors })
    }
}
