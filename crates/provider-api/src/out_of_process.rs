use crate::protocol::{OpenWorkspace, ParseRequest, Query, UpdateText};
use crate::provider::{
    Capabilities, FileMatcher, LanguageProvider, ParseError, ParseResult, ProviderError, SourceFile,
};
use crate::proxy::{ProxyError, WorkerConfig, WorkerProxy};
use crate::snippets::LazyRules;
use core_config::OptionSchema;
use core_format::{DefaultFormatter, NodeFormatter};
use core_rules::{Family, Rule};
use core_semantics::{BehaviorSummaries, CompilerDiagnostic, FileSemantics, Layers};
use core_syntax::grammars::LazyGrammar;
use core_syntax::{Grammar, LanguageId, SyntaxKind, SyntaxNode};

/// A provider whose parser runs in a worker process.
pub struct OutOfProcessProvider {
    grammar: LazyGrammar,
    matchers: Vec<FileMatcher>,
    proxy: std::sync::Arc<WorkerProxy>,
    formatter: Box<dyn NodeFormatter>,
    schema: OptionSchema,
    rules: LazyRules,
    body_kinds: &'static [SyntaxKind],
    families: &'static [Family],
    summaries: BehaviorSummaries,
}

impl OutOfProcessProvider {
    /// The worker is started lazily, on the first call that needs it.
    pub fn new(grammar: LazyGrammar, matchers: Vec<FileMatcher>, worker: WorkerConfig) -> Self {
        OutOfProcessProvider {
            grammar,
            matchers,
            proxy: std::sync::Arc::new(WorkerProxy::new(worker)),
            formatter: Box::new(DefaultFormatter),
            schema: OptionSchema::default(),
            rules: LazyRules::own(&[]),
            body_kinds: &[],
            families: &[],
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
        self.rules = LazyRules::own(rules);
        self.body_kinds = body_kinds;
        self
    }

    /// The built-in behavior summaries of the language's standard library.
    pub fn with_summaries(mut self, summaries: BehaviorSummaries) -> Self {
        self.summaries = summaries;
        self
    }

    /// The rule families the built-in pattern files may name; call before
    /// [`with_pattern_rules`](Self::with_pattern_rules).
    pub fn with_families(mut self, families: &'static [Family]) -> Self {
        self.families = families;
        self
    }

    /// The language's built-in pattern rules (the `rules/*.yaml` files, as text), added to its
    /// own. They are read when the rules are first asked for and their patterns compile when a
    /// rule first runs, so building them starts no worker.
    pub fn with_pattern_rules(mut self, sources: &'static [&'static str]) -> Self {
        let parser = std::sync::Arc::new(WorkerSnippets {
            grammar: self.grammar,
            proxy: self.proxy.clone(),
        });
        self.rules = LazyRules::new(
            self.rules.rust(),
            self.grammar.language,
            sources,
            self.families,
            parser,
        );
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
        self.grammar.id
    }

    fn language(&self) -> &str {
        self.grammar.language
    }

    fn grammar(&self) -> &Grammar {
        self.grammar.get()
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        self.proxy.capabilities().unwrap_or(Capabilities::NONE)
    }

    fn embeddable(&self) -> bool {
        false
    }

    fn formatter(&self) -> &dyn NodeFormatter {
        self.formatter.as_ref()
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn rules(&self) -> &'static [&'static dyn Rule] {
        self.rules.all()
    }

    fn rust_rules(&self) -> &'static [&'static dyn Rule] {
        self.rules.rust()
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
        if layers.types {
            items.push("symbol_findings");
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
        parse_with(&self.proxy, file)
    }
}

fn parse_with(proxy: &WorkerProxy, file: &SourceFile) -> Result<ParseResult, ProviderError> {
    let parsed = proxy
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

/// Parses pattern snippets through a worker proxy: what a built-in pattern rule holds to compile
/// itself on first use, when the provider itself is out of reach.
pub struct WorkerSnippets {
    grammar: LazyGrammar,
    proxy: std::sync::Arc<WorkerProxy>,
}

impl core_patterns::SnippetParser for WorkerSnippets {
    fn grammar(&self) -> &Grammar {
        self.grammar.get()
    }

    fn parse(&self, text: &str) -> Result<Option<SyntaxNode>, String> {
        let parsed = parse_with(&self.proxy, &SourceFile::new("builtin-pattern", 1, text))
            .map_err(|e| e.to_string())?;
        Ok(parsed.errors.is_empty().then_some(parsed.tree))
    }
}
