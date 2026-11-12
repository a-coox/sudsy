//! The scope model of a file, for the rules that need to know what a name binds to. oxc's
//! `Semantic` runs on a second parse of the text (its AST borrows the arena, so the pair is kept
//! together); results are read by span, which is the same in the lossless tree. It is built on the
//! first scope rule that runs on a file and shared with the others, so a run with syntax rules
//! only never pays for it.

use crate::adapter::source_type;
use core_rules::FileContext;
use core_syntax::SyntaxNode;
use oxc_allocator::Allocator;
use oxc_ast::{AstKind, ast::Program};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{NodeId, Semantic, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, Span};
use self_cell::self_cell;
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

self_cell!(
    struct Ast {
        owner: Allocator,
        #[covariant]
        dependent: Program,
    }
);

self_cell!(
    struct Sem {
        owner: Ast,
        #[not_covariant]
        dependent: Semantic,
    }
);

pub struct Model {
    sem: Sem,
    ok: bool,
    /// `.cjs`: the top scope is a function scope with `require`, `module` and `exports`.
    commonjs: bool,
    typescript: bool,
    /// `.d.ts`, `.d.cts`, `.d.mts`: every declaration is ambient.
    definition: bool,
    /// The binding identifier nodes by start offset, built when a rule asks for one.
    bindings: OnceCell<HashMap<u32, NodeId>>,
    jsx: OnceCell<bool>,
}

/// What a scope rule reads: oxc's `Semantic` and the file's kind.
pub struct Cx<'s, 'a> {
    pub s: &'s Semantic<'a>,
    pub commonjs: bool,
    pub ts: bool,
    pub definition: bool,
    bindings: &'s OnceCell<HashMap<u32, NodeId>>,
    jsx: &'s OnceCell<bool>,
}

impl Model {
    pub fn with<R>(&self, f: impl FnOnce(&Cx<'_, '_>) -> R) -> R {
        self.sem.with_dependent(|_, s| {
            f(&Cx {
                s,
                commonjs: self.commonjs,
                ts: self.typescript,
                definition: self.definition,
                bindings: &self.bindings,
                jsx: &self.jsx,
            })
        })
    }
}

impl<'a> Cx<'_, 'a> {
    pub fn kind(&self, id: NodeId) -> AstKind<'a> {
        self.s.nodes().kind(id)
    }

    pub fn parent(&self, id: NodeId) -> AstKind<'a> {
        self.s.nodes().parent_kind(id)
    }

    pub fn span(&self, id: NodeId) -> Span {
        self.kind(id).span()
    }

    /// The node of the binding identifier that starts at `start`.
    pub fn binding(&self, start: u32) -> Option<NodeId> {
        let map = self.bindings.get_or_init(|| {
            (self.s.nodes().iter_enumerated())
                .filter(|(_, n)| matches!(n.kind(), AstKind::BindingIdentifier(_)))
                .map(|(id, n)| (n.kind().span().start, id))
                .collect()
        });
        map.get(&start).copied()
    }

    /// Does the file have a JSX element? typescript-eslint reads that as a use of `React`.
    pub fn has_jsx(&self) -> bool {
        *self.jsx.get_or_init(|| {
            (self.s.nodes().iter()).any(|n| matches!(n.kind(), AstKind::JSXOpeningElement(_)))
        })
    }

    /// Is the import `name` the JSX pragma that the elements of the file use (TypeScript files only)?
    pub fn jsx_pragma(&self, name: &str) -> bool {
        self.ts && name == "React" && self.has_jsx()
    }

    /// The declarations of a symbol in source order: the name's span and the declaration node.
    pub fn decls(&self, sid: SymbolId) -> Vec<(Span, NodeId)> {
        let sc = self.s.scoping();
        let redecl = sc.symbol_redeclarations(sid);
        if redecl.is_empty() {
            vec![(sc.symbol_span(sid), sc.symbol_declaration(sid))]
        } else {
            redecl.iter().map(|r| (r.span, r.declaration)).collect()
        }
    }
}

/// `cargo test --release -p lang-js -- --ignored model_cost --nocapture`: what building the model
/// costs on the files of `bench/inputs/js` (best of 50 per file), next to oxc's parse alone.
#[cfg(test)]
mod cost {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore = "prints timings"]
    fn model_cost() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../bench/inputs/js");
        let (mut bytes, mut parse, mut semantic) = (0, 0.0, 0.0);
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            let st = source_type(&path.to_string_lossy());
            let (mut p, mut s) = (f64::MAX, f64::MAX);
            for _ in 0..50 {
                let start = Instant::now();
                let ast = Ast::new(Allocator::default(), |alloc| {
                    let options = ParseOptions {
                        allow_return_outside_function: true,
                        preserve_parens: false,
                        ..ParseOptions::default()
                    };
                    Parser::new(alloc, alloc.alloc_str(&text), st)
                        .with_options(options)
                        .parse()
                        .program
                });
                let parsed = Instant::now();
                let sem = Sem::new(ast, |ast| {
                    SemanticBuilder::new()
                        .with_build_nodes(true)
                        .build(ast.borrow_dependent())
                        .semantic
                });
                p = p.min((parsed - start).as_secs_f64());
                s = s.min(parsed.elapsed().as_secs_f64());
                drop(sem);
            }
            bytes += text.len();
            parse += p;
            semantic += s;
        }
        println!(
            "{bytes} bytes: oxc parse {:.2} ms, semantic build {:.2} ms",
            parse * 1e3,
            semantic * 1e3
        );
    }
}

thread_local! {
    static CACHE: RefCell<Option<(SyntaxNode, Rc<Model>)>> = const { RefCell::new(None) };
}

/// The model of the file, built once and shared by the rules that run on it; `None` when the
/// text has a syntax error (the parser's diagnostic stands, a half-read tree binds nothing).
pub fn model(file: &FileContext<'_>) -> Option<Rc<Model>> {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((root, model)) = &*cache
            && root == file.root
        {
            return model.ok.then(|| model.clone());
        }
        let name = file.path.to_string_lossy();
        let st = source_type(&name);
        let mut ok = true;
        let ast = Ast::new(Allocator::default(), |alloc| {
            let options = ParseOptions {
                allow_return_outside_function: true,
                preserve_parens: false,
                ..ParseOptions::default()
            };
            let parsed = Parser::new(alloc, alloc.alloc_str(file.text), st)
                .with_options(options)
                .parse();
            ok = parsed.diagnostics.is_empty();
            parsed.program
        });
        let sem = Sem::new(ast, |ast| {
            SemanticBuilder::new()
                .with_build_nodes(true)
                .build(ast.borrow_dependent())
                .semantic
        });
        let model = Rc::new(Model {
            sem,
            ok,
            commonjs: name.ends_with(".cjs"),
            typescript: st.is_typescript(),
            definition: [".d.ts", ".d.cts", ".d.mts"]
                .iter()
                .any(|e| name.ends_with(e)),
            bindings: OnceCell::new(),
            jsx: OnceCell::new(),
        });
        *cache = Some((file.root.clone(), model.clone()));
        ok.then_some(model)
    })
}
