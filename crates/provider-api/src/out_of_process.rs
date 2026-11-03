use crate::protocol::ParseRequest;
use crate::provider::{
    Capabilities, FileMatcher, LanguageProvider, ParseError, ParseResult, ProviderError, SourceFile,
};
use crate::proxy::{ProxyError, WorkerConfig, WorkerProxy};
use core_config::OptionSchema;
use core_format::{DefaultFormatter, NodeFormatter};
use core_rules::Rule;
use core_syntax::{Grammar, LanguageId, SyntaxKind, SyntaxNode};

/// A provider whose parser runs in a worker process.
pub struct OutOfProcessProvider {
    id: LanguageId,
    grammar: &'static Grammar,
    matchers: Vec<FileMatcher>,
    proxy: WorkerProxy,
    formatter: Box<dyn NodeFormatter>,
    schema: OptionSchema,
    rules: &'static [&'static dyn Rule],
    body_kinds: &'static [SyntaxKind],
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
            formatter: Box::new(DefaultFormatter),
            schema: OptionSchema::default(),
            rules: &[],
            body_kinds: &[],
        }
    }

    /// The provider's formatting code and its format options; without them every node uses the
    /// tag defaults and there are no options.
    pub fn with_format(mut self, formatter: Box<dyn NodeFormatter>, schema: OptionSchema) -> Self {
        self.formatter = formatter;
        self.schema = schema;
        self
    }

    /// The provider's own rules and the node kinds that are bodies (for body-scope rules).
    pub fn with_rules(
        mut self,
        rules: &'static [&'static dyn Rule],
        body_kinds: &'static [SyntaxKind],
    ) -> Self {
        self.rules = rules;
        self.body_kinds = body_kinds;
        self
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
        self.formatter.as_ref()
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn rules(&self) -> &'static [&'static dyn Rule] {
        self.rules
    }

    fn body_kinds(&self) -> &'static [SyntaxKind] {
        self.body_kinds
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
