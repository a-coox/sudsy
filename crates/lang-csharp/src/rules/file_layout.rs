//! Rules about what a file holds: `single-class-per-file`, `single-namespace-per-file` and
//! `file-name-matches-type`. They look at the file's top level only.

use super::{kind, text_of};
use core_config::Level;
use core_rules::tree::significant_range;
use core_rules::{Category, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

pub struct SingleClassPerFile;
pub struct SingleNamespacePerFile;
pub struct FileNameMatchesType;

static SINGLE_CLASS_PER_FILE: RuleMeta = RuleMeta {
    id: "single-class-per-file",
    category: Category::Style,
    description: "A file holds one class or record: every later one is reported, more parts of \
                  the first class (when it is partial) excepted. Structs, interfaces, enums and \
                  delegates are not counted. No fix. Equivalent to StyleCop SA1402.",
    example: "class A { } class B { }   // move B to its own file",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

static SINGLE_NAMESPACE_PER_FILE: RuleMeta = RuleMeta {
    id: "single-namespace-per-file",
    category: Category::Style,
    description: "A file declares one namespace: every later one, nested ones included, is \
                  reported. No fix. Equivalent to StyleCop SA1403.",
    example: "namespace A { } namespace B { }   // move B to its own file",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

static FILE_NAME_MATCHES_TYPE: RuleMeta = RuleMeta {
    id: "file-name-matches-type",
    category: Category::Style,
    description: "The file name, up to its first dot and ignoring case, is the name of the first \
                  type in the file; a generic type may also be written `Name{T,U}`. Not checked \
                  when that type is partial. No fix. Equivalent to StyleCop SA1649.",
    example: "// Orders.cs\nclass Order { }   // rename the file Order.cs",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

const CLASSES: &[SyntaxKind] = &[kind::CLASS_DECLARATION, kind::RECORD_DECLARATION];

const TYPES: &[SyntaxKind] = &[
    kind::CLASS_DECLARATION,
    kind::RECORD_DECLARATION,
    kind::STRUCT_DECLARATION,
    kind::RECORD_STRUCT_DECLARATION,
    kind::INTERFACE_DECLARATION,
    kind::ENUM_DECLARATION,
    kind::DELEGATE_DECLARATION,
];

/// The type declarations and namespaces that are not inside another type, in source order.
fn collect_top_level(
    node: &SyntaxNode,
    types: &mut Vec<SyntaxNode>,
    namespaces: &mut Vec<SyntaxNode>,
) {
    for child in node.children() {
        let kind = child.kind();
        if kind == kind::NAMESPACE_DECLARATION || kind == kind::FILE_SCOPED_NAMESPACE_DECLARATION {
            namespaces.push(child.clone());
            collect_top_level(&child, types, namespaces);
        } else if TYPES.contains(&kind) {
            types.push(child);
        }
    }
}

fn top_level(root: &SyntaxNode) -> (Vec<SyntaxNode>, Vec<SyntaxNode>) {
    let (mut types, mut namespaces) = (Vec::new(), Vec::new());
    collect_top_level(root, &mut types, &mut namespaces);
    (types, namespaces)
}

fn name_token(declaration: &SyntaxNode) -> Option<SyntaxToken> {
    declaration
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind::IDENTIFIER_TOKEN)
}

fn is_partial(declaration: &SyntaxNode) -> bool {
    declaration
        .children_with_tokens()
        .any(|e| e.kind() == kind::PARTIAL_KEYWORD)
}

impl Rule for SingleClassPerFile {
    fn meta(&self) -> &'static RuleMeta {
        &SINGLE_CLASS_PER_FILE
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::COMPILATION_UNIT]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut first: Option<String> = None;
        for class in top_level(node)
            .0
            .iter()
            .filter(|t| CLASSES.contains(&t.kind()))
        {
            let Some(name) = name_token(class) else {
                continue;
            };
            if is_partial(class) && first.as_deref() == Some(name.text()) {
                continue;
            }
            if first.is_some() {
                ctx.report(name.text_range(), "a file may only contain a single class");
            }
            first.get_or_insert_with(|| name.text().to_string());
        }
    }
}

impl Rule for SingleNamespacePerFile {
    fn meta(&self) -> &'static RuleMeta {
        &SINGLE_NAMESPACE_PER_FILE
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::COMPILATION_UNIT]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        for namespace in top_level(node).1.iter().skip(1) {
            if let Some(name) = namespace.children().next() {
                ctx.report(
                    significant_range(&name, ctx.file.grammar),
                    "a file may only contain a single namespace",
                );
            }
        }
    }
}

impl Rule for FileNameMatchesType {
    fn meta(&self) -> &'static RuleMeta {
        &FILE_NAME_MATCHES_TYPE
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::COMPILATION_UNIT]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(first) = top_level(node).0.into_iter().next() else {
            return;
        };
        let (Some(name), false) = (name_token(&first), is_partial(&first)) else {
            return;
        };
        let Some(file) = ctx.file.path.file_name().and_then(|f| f.to_str()) else {
            return;
        };
        let stem = file.split('.').next().unwrap_or(file);
        let generic = first
            .children()
            .find(|c| c.kind() == kind::TYPE_PARAMETER_LIST)
            .map(|list| {
                let names: Vec<&str> = list.children().map(|p| text_of(&p, ctx)).collect();
                format!("{}{{{}}}", name.text(), names.join(","))
            });
        let matches = stem.eq_ignore_ascii_case(name.text())
            || generic.is_some_and(|g| stem.eq_ignore_ascii_case(&g));
        if !matches {
            ctx.report(
                name.text_range(),
                "the file name should match the first type in it",
            );
        }
    }
}
