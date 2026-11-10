//! `member-order` and `accessor-order`: the order of the members of a type, namespace or file,
//! and of the accessors of a property or event. Both report only; the report places match
//! StyleCop SA1201 to SA1204, SA1212, SA1213 and SA1214.

use super::kind;
use core_rules::rule;
use core_rules::tree::{code_start, entry_range, reorder_edits, significant_range};
use core_rules::{Check, Opt, OptionDecl, RuleContext};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxNodeExt, TextRange};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Access {
    NotSpecified,
    Private,
    PrivateProtected,
    Protected,
    ProtectedInternal,
    Internal,
    Public,
}

impl Access {
    fn name(self) -> &'static str {
        match self {
            Access::NotSpecified => "unspecified access",
            Access::Private => "private",
            Access::PrivateProtected => "private protected",
            Access::Protected => "protected",
            Access::ProtectedInternal => "protected internal",
            Access::Internal => "internal",
            Access::Public => "public",
        }
    }
}

/// The access the modifiers declare, in the order StyleCop reads them.
fn access_of(modifiers: &[SyntaxKind]) -> Access {
    let (mut private, mut protected, mut internal) = (false, false, false);
    for &m in modifiers {
        match m {
            kind::PUBLIC_KEYWORD => return Access::Public,
            kind::PRIVATE_KEYWORD if protected => return Access::PrivateProtected,
            kind::PRIVATE_KEYWORD => private = true,
            kind::INTERNAL_KEYWORD if protected => return Access::ProtectedInternal,
            kind::INTERNAL_KEYWORD => internal = true,
            kind::PROTECTED_KEYWORD if internal => return Access::ProtectedInternal,
            kind::PROTECTED_KEYWORD if private => return Access::PrivateProtected,
            kind::PROTECTED_KEYWORD => protected = true,
            _ => {}
        }
    }
    match (private, protected, internal) {
        (true, ..) => Access::Private,
        (_, true, _) => Access::Protected,
        (.., true) => Access::Internal,
        _ => Access::NotSpecified,
    }
}

struct Member {
    node: SyntaxNode,
    /// The kind for ordering: an event field is an event, a record a class.
    kind: SyntaxKind,
    access: Access,
    is_static: bool,
    is_const: bool,
    is_readonly: bool,
}

impl Member {
    fn new(node: SyntaxNode, container: SyntaxKind, g: &Grammar) -> Member {
        let modifiers: Vec<SyntaxKind> = node
            .tokens()
            .map(|t| t.kind())
            .filter(|&k| g.has_tag(k, "modifier"))
            .collect();
        let has = |k| modifiers.contains(&k);
        let explicit = node
            .children()
            .any(|c| c.kind() == kind::EXPLICIT_INTERFACE_SPECIFIER);
        let node_kind = node.kind();
        let access = if (node_kind == kind::CONSTRUCTOR_DECLARATION && has(kind::STATIC_KEYWORD))
            || (explicit
                && matches!(
                    node_kind,
                    kind::METHOD_DECLARATION
                        | kind::PROPERTY_DECLARATION
                        | kind::INDEXER_DECLARATION
                )) {
            Access::Public
        } else {
            match access_of(&modifiers) {
                Access::NotSpecified => match container {
                    kind::COMPILATION_UNIT
                    | kind::NAMESPACE_DECLARATION
                    | kind::FILE_SCOPED_NAMESPACE_DECLARATION => Access::Internal,
                    kind::INTERFACE_DECLARATION => Access::Public,
                    _ => Access::Private,
                },
                a => a,
            }
        };
        Member {
            kind: match node_kind {
                kind::EVENT_FIELD_DECLARATION => kind::EVENT_DECLARATION,
                kind::RECORD_DECLARATION => kind::CLASS_DECLARATION,
                kind::RECORD_STRUCT_DECLARATION => kind::STRUCT_DECLARATION,
                k => k,
            },
            node,
            access,
            is_static: has(kind::STATIC_KEYWORD),
            is_const: has(kind::CONST_KEYWORD),
            is_readonly: has(kind::READ_ONLY_KEYWORD),
        }
    }

    /// Const members are static and readonly as well.
    fn implied_static(&self) -> bool {
        self.is_static || self.is_const
    }

    fn implied_readonly(&self) -> bool {
        self.is_readonly || self.is_const
    }
}

/// Where StyleCop reports a member: its name, or its start for members without one.
fn name_range(node: &SyntaxNode, g: &Grammar) -> TextRange {
    let token = |of: &SyntaxNode, k: SyntaxKind| of.token(k).map(|t| t.text_range());
    let found = match node.kind() {
        kind::PROPERTY_DECLARATION
        | kind::METHOD_DECLARATION
        | kind::CONSTRUCTOR_DECLARATION
        | kind::DESTRUCTOR_DECLARATION
        | kind::CLASS_DECLARATION
        | kind::STRUCT_DECLARATION
        | kind::INTERFACE_DECLARATION
        | kind::ENUM_DECLARATION
        | kind::RECORD_DECLARATION
        | kind::RECORD_STRUCT_DECLARATION
        | kind::DELEGATE_DECLARATION
        | kind::EVENT_DECLARATION => token(node, kind::IDENTIFIER_TOKEN),
        kind::FIELD_DECLARATION => node
            .child(kind::VARIABLE_DECLARATION)
            .and_then(|d| d.child(kind::VARIABLE_DECLARATOR))
            .and_then(|d| token(&d, kind::IDENTIFIER_TOKEN)),
        kind::INDEXER_DECLARATION => token(node, kind::THIS_KEYWORD),
        kind::NAMESPACE_DECLARATION => node
            .children()
            .find(|c| g.has_tag(c.kind(), "name"))
            .map(|n| significant_range(&n, g)),
        _ => None,
    };
    found.unwrap_or_else(|| code_range(node, g))
}

/// Where a node's code starts, as the range to the end of the node.
fn code_range(node: &SyntaxNode, g: &Grammar) -> TextRange {
    TextRange::new(code_start(node, g), significant_range(node, g).end())
}

const OUTER_ORDER: &[SyntaxKind] = &[
    kind::FILE_SCOPED_NAMESPACE_DECLARATION,
    kind::NAMESPACE_DECLARATION,
    kind::DELEGATE_DECLARATION,
    kind::ENUM_DECLARATION,
    kind::INTERFACE_DECLARATION,
    kind::STRUCT_DECLARATION,
    kind::CLASS_DECLARATION,
];

const TYPE_ORDER: &[SyntaxKind] = &[
    kind::FIELD_DECLARATION,
    kind::CONSTRUCTOR_DECLARATION,
    kind::DESTRUCTOR_DECLARATION,
    kind::DELEGATE_DECLARATION,
    kind::EVENT_DECLARATION,
    kind::ENUM_DECLARATION,
    kind::INTERFACE_DECLARATION,
    kind::PROPERTY_DECLARATION,
    kind::INDEXER_DECLARATION,
    kind::CONVERSION_OPERATOR_DECLARATION,
    kind::OPERATOR_DECLARATION,
    kind::METHOD_DECLARATION,
    kind::STRUCT_DECLARATION,
    kind::CLASS_DECLARATION,
];

/// The kinds `access` compares.
const ACCESS_KINDS: &[SyntaxKind] = &[
    kind::DELEGATE_DECLARATION,
    kind::ENUM_DECLARATION,
    kind::INTERFACE_DECLARATION,
    kind::STRUCT_DECLARATION,
    kind::CLASS_DECLARATION,
    kind::FIELD_DECLARATION,
    kind::CONSTRUCTOR_DECLARATION,
    kind::EVENT_DECLARATION,
    kind::PROPERTY_DECLARATION,
    kind::INDEXER_DECLARATION,
    kind::METHOD_DECLARATION,
    kind::CONVERSION_OPERATOR_DECLARATION,
    kind::OPERATOR_DECLARATION,
];

fn kind_name(k: SyntaxKind) -> &'static str {
    match k {
        kind::NAMESPACE_DECLARATION | kind::FILE_SCOPED_NAMESPACE_DECLARATION => "namespace",
        kind::DELEGATE_DECLARATION => "delegate",
        kind::ENUM_DECLARATION => "enum",
        kind::INTERFACE_DECLARATION => "interface",
        kind::STRUCT_DECLARATION => "struct",
        kind::CLASS_DECLARATION => "class",
        kind::FIELD_DECLARATION => "field",
        kind::CONSTRUCTOR_DECLARATION => "constructor",
        kind::DESTRUCTOR_DECLARATION => "destructor",
        kind::EVENT_DECLARATION => "event",
        kind::PROPERTY_DECLARATION => "property",
        kind::INDEXER_DECLARATION => "indexer",
        kind::METHOD_DECLARATION => "method",
        kind::CONVERSION_OPERATOR_DECLARATION => "conversion",
        kind::OPERATOR_DECLARATION => "operator",
        _ => "member",
    }
}

fn position(order: &[SyntaxKind], k: SyntaxKind) -> i32 {
    order.iter().position(|&o| o == k).map_or(-1, |i| i as i32)
}

rule! {
    pub MemberOrder {
        id: "member-order",
        category: Style,
        default: Off,
        description: "Members are out of order. Compared with the member before, in the order of the \
                  `element_order` option: the kind (fields, constructors, finalizers, delegates, \
                  events, enums, interfaces, properties, indexers, operators, methods, structs, \
                  classes; in a namespace or file: namespaces, delegates, enums, interfaces, \
                  structs, classes), the access (public, internal, protected internal, protected, \
                  private protected, private), constants before other fields, static before \
                  instance members, readonly fields before the others. A trait only counts \
                  between members that agree on the traits listed before it. No fix: moving \
                  members would have to take their comments and directives along.",
        example: "class A {\n  void F() { }\n  int x;   // fields come first\n}",
        options: [OptionDecl {
            name: "element_order",
            default: Opt::List(&["kind", "accessibility", "constant", "static", "readonly"]),
            check: Check::OneOf(&["kind", "accessibility", "constant", "static", "readonly"]),
            description: "The traits that decide the order, most important first. A trait that is \
                      left out is not checked.",
        }],
        kinds: &[
            kind::COMPILATION_UNIT,
            kind::NAMESPACE_DECLARATION,
            kind::FILE_SCOPED_NAMESPACE_DECLARATION,
            kind::CLASS_DECLARATION,
            kind::STRUCT_DECLARATION,
            kind::INTERFACE_DECLARATION,
            kind::RECORD_DECLARATION,
            kind::RECORD_STRUCT_DECLARATION,
        ],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let container = node.kind();
        let is_type = !matches!(
            container,
            kind::COMPILATION_UNIT
                | kind::NAMESPACE_DECLARATION
                | kind::FILE_SCOPED_NAMESPACE_DECLARATION
        );
        let members: Vec<Member> = node
            .children()
            .filter(|c| g.has_tag(c.kind(), "member"))
            .map(|c| Member::new(c, container, g))
            .collect();
        let mut traits = ctx.options.list("element_order");
        if traits.is_empty() {
            traits = vec!["kind", "accessibility", "constant", "static", "readonly"];
        }
        let before = |t: &str| {
            let at = traits.iter().position(|x| *x == t)?;
            Some(traits[..at].to_vec())
        };
        let mut found: Vec<(TextRange, String)> = Vec::new();
        if let Some(prior) = before("kind") {
            kinds_in_order(&members, &prior, is_type, g, &mut found);
        }
        if let Some(prior) = before("accessibility") {
            access_in_order(&members, &prior, g, &mut found);
        }
        if let (Some(prior), true) = (before("constant"), is_type) {
            constants_first(&members, &prior, g, &mut found);
        }
        if let Some(prior) = before("static") {
            statics_first(&members, &prior, g, &mut found);
        }
        if let (Some(prior), true) = (before("readonly"), is_type) {
            readonly_first(&members, &prior, g, &mut found);
        }
        for (range, message) in found {
            ctx.report(range, message);
        }
    }
}

fn kinds_in_order(
    members: &[Member],
    prior: &[&str],
    is_type: bool,
    g: &Grammar,
    out: &mut Vec<(TextRange, String)>,
) {
    let order = if is_type { TYPE_ORDER } else { OUTER_ORDER };
    let mut i = 0;
    while i + 1 < members.len() {
        let (a, b) = (&members[i], &members[i + 1]);
        i += 1;
        if b.node.kind() == kind::INCOMPLETE_MEMBER {
            i += 1;
            continue;
        }
        if a.node.kind() == kind::INCOMPLETE_MEMBER {
            continue;
        }
        let comparable = prior.iter().all(|t| match *t {
            "accessibility" => a.access == b.access,
            "static" => a.is_static == b.is_static,
            _ => true,
        });
        if comparable && position(order, a.kind) > position(order, b.kind) {
            out.push((
                name_range(&b.node, g),
                format!(
                    "{} should not follow {}",
                    kind_name(b.kind),
                    kind_name(a.kind)
                ),
            ));
        }
    }
}

fn access_in_order(
    members: &[Member],
    prior: &[&str],
    g: &Grammar,
    out: &mut Vec<(TextRange, String)>,
) {
    let mut previous: Option<&Member> = None;
    for m in members.iter().filter(|m| ACCESS_KINDS.contains(&m.kind)) {
        if let Some(p) = previous {
            let comparable = prior.iter().all(|t| match *t {
                "kind" => p.kind == m.kind,
                "constant" => p.is_const == m.is_const,
                "readonly" => p.is_readonly == m.is_readonly,
                "static" => p.is_static == m.is_static,
                _ => true,
            });
            if comparable && m.access > p.access {
                out.push((
                    name_range(&m.node, g),
                    format!(
                        "{} members should come before {} members",
                        m.access.name(),
                        p.access.name()
                    ),
                ));
            }
        }
        previous = Some(m);
    }
}

fn constants_first(
    members: &[Member],
    prior: &[&str],
    g: &Grammar,
    out: &mut Vec<(TextRange, String)>,
) {
    // Before the first field: a constant, with no access, static or readonly in common.
    let (mut constant, mut is_static, mut readonly, mut access) =
        (true, false, false, Access::NotSpecified);
    for m in members
        .iter()
        .filter(|m| m.node.kind() == kind::FIELD_DECLARATION)
    {
        let comparable = prior.iter().all(|t| match *t {
            "accessibility" => m.access == access,
            "readonly" => m.implied_readonly() == readonly,
            "static" => m.implied_static() == is_static,
            _ => true,
        });
        if comparable && !constant && m.is_const {
            out.push((
                name_range(&m.node, g),
                "constants should come before fields".into(),
            ));
        }
        (constant, is_static, readonly, access) = (
            m.is_const,
            m.implied_static(),
            m.implied_readonly(),
            m.access,
        );
    }
}

fn statics_first(
    members: &[Member],
    prior: &[&str],
    g: &Grammar,
    out: &mut Vec<(TextRange, String)>,
) {
    let mut previous: Option<&Member> = None;
    for m in members {
        if let Some(p) = previous {
            let comparable = prior.iter().all(|t| match *t {
                "accessibility" => m.access == p.access,
                "readonly" => m.implied_readonly() == p.implied_readonly(),
                "constant" => m.is_const == p.is_const,
                "kind" => m.kind == p.kind,
                _ => true,
            });
            if comparable && m.implied_static() && !p.implied_static() {
                out.push((
                    name_range(&m.node, g),
                    format!(
                        "static {} elements should come before instance elements",
                        m.access.name()
                    ),
                ));
            }
        }
        previous = Some(m);
    }
}

fn readonly_first(
    members: &[Member],
    prior: &[&str],
    g: &Grammar,
    out: &mut Vec<(TextRange, String)>,
) {
    let mut previous: Option<&Member> = None;
    for m in members {
        if m.node.kind() != kind::FIELD_DECLARATION {
            previous = None;
            continue;
        }
        if let Some(p) = previous {
            let comparable = prior.iter().all(|t| match *t {
                "accessibility" => m.access == p.access,
                "constant" => m.is_const == p.is_const,
                "static" => m.implied_static() == p.implied_static(),
                _ => true,
            });
            if comparable && m.implied_readonly() && !p.implied_readonly() {
                out.push((
                    name_range(&m.node, g),
                    "readonly fields should come before non-readonly fields".into(),
                ));
            }
        }
        previous = Some(m);
    }
}

rule! {
    pub AccessorOrder {
        id: "accessor-order",
        category: Style,
        default: Off,
        fix: Safe,
        description: "`get` goes before `set` or `init`, and `add` before `remove`. The fix swaps \
                  the two accessors (not offered when a comment stands between or in them).",
        example: "int P { set { } get { return 1; } }   // get first",
        kinds: &[
            kind::PROPERTY_DECLARATION,
            kind::INDEXER_DECLARATION,
            kind::EVENT_DECLARATION,
        ],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let Some(list) = node.child(kind::ACCESSOR_LIST) else {
            return;
        };
        let accessors: Vec<SyntaxNode> = list
            .children()
            .filter(|c| g.has_tag(c.kind(), "accessor-declaration"))
            .collect();
        let [first, second] = accessors.as_slice() else {
            return;
        };
        let (range, message) = match (first.kind(), second.kind()) {
            (
                kind::SET_ACCESSOR_DECLARATION | kind::INIT_ACCESSOR_DECLARATION,
                kind::GET_ACCESSOR_DECLARATION,
            ) => (
                code_range(first, g),
                "the get accessor should come before the set accessor",
            ),
            (kind::REMOVE_ACCESSOR_DECLARATION, kind::ADD_ACCESSOR_DECLARATION) => (
                first
                    .tokens()
                    .find(|t| {
                        !g.is_trivia(t.kind())
                            && g.has_tag(t.kind(), "accessor-declaration-keyword")
                    })
                    .map_or_else(|| code_range(first, g), |t| t.text_range()),
                "the add accessor should come before the remove accessor",
            ),
            _ => return,
        };
        let ranges = [entry_range(first, g), entry_range(second, g)];
        let (text, span) = (
            ctx.file.text,
            TextRange::new(ranges[0].start(), ranges[1].end()),
        );
        let commented = list
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| span.contains_range(t.text_range()) && g.has_tag(t.kind(), "comment"));
        let fix = (!commented).then(|| ("Swap the accessors", reorder_edits(text, &ranges, &[1, 0])));
        ctx.report_fix(range, message, fix);
    }
}
