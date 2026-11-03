use crate::protocol::{OpenWorkspace, ParseRequest, Query, UpdateText};
use crate::provider::{
    Capabilities, FileMatcher, LanguageProvider, ParseError, ParseResult, ProviderError, SourceFile,
};
use crate::proxy::{ProxyError, WorkerConfig, WorkerProxy};
use core_config::OptionSchema;
use core_format::{DefaultFormatter, NodeFormatter};
use core_rules::Rule;
use core_semantics::{BehaviorSummaries, CompilerDiagnostic, FileSemantics, Layers};
use core_syntax::{Grammar, LanguageId, SyntaxKind, SyntaxNode};

/// A provider whose parser runs in a worker process.
pub struct OutOfProcessProvider {
    id: LanguageId,
    grammar: &'static Grammar,
    matchers: Vec<FileMatcher>,
    proxy: std::sync::Arc<WorkerProxy>,
    formatter: Box<dyn NodeFormatter>,
    schema: OptionSchema,
    rules: &'static [&'static dyn Rule],
    body_kinds: &'static [SyntaxKind],
    summaries: BehaviorSummaries,
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
            proxy: std::sync::Arc::new(WorkerProxy::new(worker)),
            formatter: Box::new(DefaultFormatter),
            schema: OptionSchema::default(),
            rules: &[],
            body_kinds: &[],
            summaries: BehaviorSummaries::default(),
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

    /// The built-in behavior summaries of the language's standard library.
    pub fn with_summaries(mut self, summaries: BehaviorSummaries) -> Self {
        self.summaries = summaries;
        self
    }

    pub fn proxy(&self) -> &WorkerProxy {
        &self.proxy
    }
}

fn to_provider_error(e: ProxyError) -> ProviderError {
    match e {
        ProxyError::Unavailable(m) => ProviderError::Unavailable(m),
        ProxyError::RuntimeMissing(m) => ProviderError::RuntimeMissing(m),
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

    fn summaries(&self) -> BehaviorSummaries {
        self.summaries.clone()
    }

    fn compiler_diagnostics(
        &self,
        file: &SourceFile,
        ids: &[&str],
    ) -> Result<Vec<CompilerDiagnostic>, ProviderError> {
        let update = UpdateText {
            file_id: file.id.clone(),
            version: file.version,
            text: file.text.clone(),
        };
        let query = Query {
            file_id: file.id.clone(),
            version: file.version,
            items: vec!["diagnostics".into()],
            diagnostic_ids: ids.iter().map(|s| s.to_string()).collect(),
        };
        let (answers, _) = self
            .proxy
            .analyze(update, Some(query), false)
            .map_err(to_provider_error)?;
        Ok(answers.map(|a| a.diagnostics).unwrap_or_default())
    }

    fn prestart(&self) {
        self.proxy.prestart();
    }

    fn open_workspace(&self, paths: &[std::path::PathBuf]) {
        self.proxy.open_workspace(OpenWorkspace {
            paths: paths
                .iter()
                // Absolute: the worker does not run in the caller's working directory.
                .map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone()))
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            properties: Default::default(),
        });
    }

    fn semantics(
        &self,
        file: &SourceFile,
        layers: Layers,
    ) -> Result<Option<FileSemantics>, ProviderError> {
        let caps = self.capabilities();
        let layers = Layers {
            binding: layers.binding && caps.binding,
            types: layers.types && caps.types,
            values: layers.values && caps.values,
            flow: layers.flow && caps.flow,
            project: layers.project && caps.project,
        };
        if layers.is_empty() {
            return Ok(None);
        }
        // Each layer is answered by one kind of query item.
        let mut items: Vec<&str> = Vec::new();
        if layers.binding {
            items.push("unused_imports");
        }
        if layers.types || layers.project {
            items.push("declarations");
        }
        if layers.project {
            items.push("references");
        }
        if layers.values {
            items.push("maybe_null");
        }
        let query = (!items.is_empty()).then(|| Query {
            file_id: file.id.clone(),
            version: file.version,
            items: items.into_iter().map(String::from).collect(),
            diagnostic_ids: Vec::new(),
        });
        let update = UpdateText {
            file_id: file.id.clone(),
            version: file.version,
            text: file.text.clone(),
        };
        let (answers, graphs) = self
            .proxy
            .analyze(update, query, layers.flow)
            .map_err(to_provider_error)?;
        Ok(Some(FileSemantics {
            answers: answers.unwrap_or_default(),
            graphs: graphs.map(|g| g.bodies).unwrap_or_default(),
        }))
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
