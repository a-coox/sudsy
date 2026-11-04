//! What the documentation rules (`doc_*`) read: the documentation comment of a declaration as a
//! list of top-level XML elements, and the few facts about the declaration they need (kind,
//! accessibility, where to report). Syntax only: the rules agree with StyleCop for everything
//! that does not need name lookup (`<include>` files, `<inheritdoc>` targets, type aliases).

use super::kind;
use core_rules::RuleContext;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

/// The kinds a rule that reads documentation comments subscribes to.
pub const DOC_KINDS: &[SyntaxKind] = &[
    kind::SINGLE_LINE_DOCUMENTATION_COMMENT_TRIVIA,
    kind::MULTI_LINE_DOCUMENTATION_COMMENT_TRIVIA,
];

/// The declarations that can be documented.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decl {
    Class,
    Struct,
    Interface,
    Record,
    RecordStruct,
    Enum,
    Delegate,
    Method,
    Constructor,
    Destructor,
    Operator,
    Conversion,
    Property,
    Indexer,
    Event,
    EventField,
    Field,
    EnumMember,
}

impl Decl {
    pub fn of(kind: SyntaxKind) -> Option<Decl> {
        Some(match kind {
            kind::CLASS_DECLARATION => Decl::Class,
            kind::STRUCT_DECLARATION => Decl::Struct,
            kind::INTERFACE_DECLARATION => Decl::Interface,
            kind::RECORD_DECLARATION => Decl::Record,
            kind::RECORD_STRUCT_DECLARATION => Decl::RecordStruct,
            kind::ENUM_DECLARATION => Decl::Enum,
            kind::DELEGATE_DECLARATION => Decl::Delegate,
            kind::METHOD_DECLARATION => Decl::Method,
            kind::CONSTRUCTOR_DECLARATION => Decl::Constructor,
            kind::DESTRUCTOR_DECLARATION => Decl::Destructor,
            kind::OPERATOR_DECLARATION => Decl::Operator,
            kind::CONVERSION_OPERATOR_DECLARATION => Decl::Conversion,
            kind::PROPERTY_DECLARATION => Decl::Property,
            kind::INDEXER_DECLARATION => Decl::Indexer,
            kind::EVENT_DECLARATION => Decl::Event,
            kind::EVENT_FIELD_DECLARATION => Decl::EventField,
            kind::FIELD_DECLARATION => Decl::Field,
            kind::ENUM_MEMBER_DECLARATION => Decl::EnumMember,
            _ => return None,
        })
    }

    /// Class, struct, interface, record (StyleCop's `TypeDeclaration`).
    pub fn is_type(self) -> bool {
        matches!(
            self,
            Decl::Class | Decl::Struct | Decl::Interface | Decl::Record | Decl::RecordStruct
        )
    }

    /// Types and enums: what can contain members.
    pub fn is_container(self) -> bool {
        self.is_type() || self == Decl::Enum
    }

    /// Method or delegate: the declarations with a return type.
    pub fn has_return_type(self) -> bool {
        matches!(self, Decl::Method | Decl::Delegate)
    }

    /// The declarations most documentation rules apply to (StyleCop's `ElementDocumentationBase`).
    pub fn is_element(self) -> bool {
        self.is_container()
            || matches!(
                self,
                Decl::Delegate
                    | Decl::Method
                    | Decl::Constructor
                    | Decl::Operator
                    | Decl::Conversion
                    | Decl::Property
                    | Decl::Indexer
                    | Decl::Field
                    | Decl::EnumMember
            )
    }

    /// The declarations that need a summary (StyleCop's `ElementDocumentationSummaryBase`).
    pub fn takes_summary(self) -> bool {
        self.is_container()
            || matches!(
                self,
                Decl::Delegate
                    | Decl::Method
                    | Decl::Constructor
                    | Decl::Destructor
                    | Decl::Property
                    | Decl::Indexer
                    | Decl::Event
                    | Decl::EventField
                    | Decl::Field
            )
    }
}

/// Accessibility in the order of Roslyn's `Accessibility`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Access {
    Private,
    PrivateProtected,
    Protected,
    Internal,
    ProtectedInternal,
    Public,
}

pub fn modifier_access(node: &SyntaxNode) -> Option<Access> {
    let (mut private, mut protected, mut internal) = (false, false, false);
    for token in node.children_with_tokens().filter_map(|e| e.into_token()) {
        match token.kind() {
            kind::PUBLIC_KEYWORD => return Some(Access::Public),
            kind::PRIVATE_KEYWORD if protected => return Some(Access::PrivateProtected),
            kind::PRIVATE_KEYWORD => private = true,
            kind::INTERNAL_KEYWORD if protected => return Some(Access::ProtectedInternal),
            kind::INTERNAL_KEYWORD => internal = true,
            kind::PROTECTED_KEYWORD if internal => return Some(Access::ProtectedInternal),
            kind::PROTECTED_KEYWORD if private => return Some(Access::PrivateProtected),
            kind::PROTECTED_KEYWORD => protected = true,
            _ => {}
        }
    }
    if private {
        Some(Access::Private)
    } else if protected {
        Some(Access::Protected)
    } else if internal {
        Some(Access::Internal)
    } else {
        None
    }
}

pub fn has_modifier(node: &SyntaxNode, modifier: SyntaxKind) -> bool {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| t.kind() == modifier)
}

fn has_child(node: &SyntaxNode, child: SyntaxKind) -> bool {
    node.children().any(|c| c.kind() == child)
}

/// The accessibility written on the declaration, or what it defaults to.
fn declared_access(node: &SyntaxNode, decl: Decl) -> Access {
    if let Some(written) = modifier_access(node) {
        return written;
    }
    let in_interface = node
        .parent()
        .is_some_and(|p| p.kind() == kind::INTERFACE_DECLARATION);
    let explicit = has_child(node, kind::EXPLICIT_INTERFACE_SPECIFIER);
    match decl {
        Decl::EnumMember => Access::Public,
        _ if in_interface => Access::Public,
        Decl::Class
        | Decl::Struct
        | Decl::Interface
        | Decl::Record
        | Decl::RecordStruct
        | Decl::Enum
        | Decl::Delegate => {
            if node
                .parent()
                .is_some_and(|p| Decl::of(p.kind()).is_some_and(Decl::is_container))
            {
                Access::Private
            } else {
                Access::Internal
            }
        }
        Decl::Method | Decl::Property | Decl::Indexer | Decl::Event if explicit => Access::Public,
        // A finalizer has no modifiers; Roslyn calls it protected.
        Decl::Destructor => Access::Protected,
        _ => Access::Private,
    }
}

fn combine(declared: Access, enclosing: Access) -> Access {
    use Access::*;
    match enclosing {
        Private => enclosing,
        PrivateProtected => match declared {
            Private => declared,
            _ => PrivateProtected,
        },
        Protected => match declared {
            Private => declared,
            PrivateProtected | Internal => PrivateProtected,
            _ => Protected,
        },
        Internal => match declared {
            Private => declared,
            PrivateProtected | Protected => PrivateProtected,
            _ => Internal,
        },
        ProtectedInternal => match declared {
            Public => ProtectedInternal,
            _ => declared,
        },
        Public => declared,
    }
}

/// The accessibility from outside the file: the declared one limited by the enclosing types.
fn effective_access(node: &SyntaxNode, decl: Decl) -> Access {
    let declared = declared_access(node, decl);
    if declared <= Access::Private && decl != Decl::EnumMember {
        return declared;
    }
    let enclosing = node.parent().and_then(|p| {
        Decl::of(p.kind())
            .filter(|d| d.is_container())
            .map(|d| (p, d))
    });
    match enclosing {
        Some((p, d)) => combine(declared, effective_access(&p, d)),
        None if decl == Decl::Delegate => Access::Internal,
        None => declared,
    }
}

/// A documentable declaration.
pub struct Declaration {
    pub node: SyntaxNode,
    pub kind: Decl,
}

/// A declaration and its documentation comment.
pub struct Documented {
    pub decl: Declaration,
    pub doc: Doc,
}

impl std::ops::Deref for Documented {
    type Target = Declaration;
    fn deref(&self) -> &Declaration {
        &self.decl
    }
}

/// The top-level elements of a documentation comment.
pub struct Doc {
    pub node: SyntaxNode,
    pub elements: Vec<Element>,
}

/// An XML element (`<a>..</a>` or `<a/>`) of a documentation comment.
#[derive(Clone)]
pub struct Element {
    pub name: String,
    pub node: SyntaxNode,
}

/// An attribute of an element.
pub struct Attr {
    pub kind: SyntaxKind,
    pub value: String,
    pub range: TextRange,
}

/// The documentation comment before `node`, if any.
pub fn doc_of(node: &SyntaxNode, grammar: &Grammar) -> Option<Doc> {
    leading_doc(node, grammar).flatten().map(Doc::new)
}

/// The first documentation comment in the leading trivia of `node`.
fn leading_doc(node: &SyntaxNode, grammar: &Grammar) -> Option<Option<SyntaxNode>> {
    for element in node.children_with_tokens() {
        if let Some(n) = element.as_node() {
            if DOC_KINDS.contains(&n.kind()) {
                return Some(Some(n.clone()));
            }
            if !grammar.is_trivia(n.kind())
                && let Some(found) = leading_doc(n, grammar)
            {
                return Some(found);
            }
        } else if let Some(t) = element.as_token()
            && !grammar.is_trivia(t.kind())
            && !t.text().is_empty()
        {
            return Some(None);
        }
    }
    None
}

/// The declaration `doc` is the documentation of: only when `doc` is the first documentation
/// comment before the declaration and `accept` takes the declaration. A comment with an
/// `<include>` element is not read (the included file is not).
pub fn documented(
    doc: &SyntaxNode,
    ctx: &RuleContext<'_>,
    accept: impl Fn(Decl) -> bool,
) -> Option<Documented> {
    let grammar = ctx.file.grammar;
    let node = doc
        .ancestors()
        .skip(1)
        .find(|n| grammar.has_tag(n.kind(), "declaration"))?;
    let kind = Decl::of(node.kind()).filter(|d| accept(*d))?;
    if leading_doc(&node, grammar).flatten().as_ref() != Some(doc) {
        return None;
    }
    let doc = Doc::new(doc.clone());
    if doc.first("include").is_some() {
        return None;
    }
    Some(Documented {
        decl: Declaration { node, kind },
        doc,
    })
}

impl Doc {
    pub fn new(node: SyntaxNode) -> Doc {
        let elements = node
            .children()
            .filter(|c| matches!(c.kind(), kind::XML_ELEMENT | kind::XML_EMPTY_ELEMENT))
            .map(Element::new)
            .collect();
        Doc { node, elements }
    }

    pub fn first(&self, name: &str) -> Option<&Element> {
        self.elements.iter().find(|e| e.name == name)
    }

    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.elements.iter().filter(move |e| e.name == name)
    }

    pub fn has_inheritdoc(&self) -> bool {
        self.first("inheritdoc").is_some()
    }

    /// Is every element and all text blank (StyleCop's `HasDocumentation` is the opposite)?
    pub fn is_blank(&self) -> bool {
        self.node.children().all(|c| is_blank(&c, false))
    }
}

fn name_of(xml_name: &SyntaxNode) -> String {
    xml_name
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == kind::IDENTIFIER_TOKEN)
        .last()
        .map(|t| t.text().to_string())
        .unwrap_or_default()
}

pub fn element_name(node: &SyntaxNode) -> String {
    let holder = if node.kind() == kind::XML_ELEMENT {
        node.children()
            .find(|c| c.kind() == kind::XML_ELEMENT_START_TAG)
    } else {
        Some(node.clone())
    };
    holder
        .and_then(|h| h.children().find(|c| c.kind() == kind::XML_NAME))
        .map(|n| name_of(&n))
        .unwrap_or_default()
}

fn is_text_token(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        kind::XML_TEXT_LITERAL_TOKEN
            | kind::XML_TEXT_LITERAL_NEW_LINE_TOKEN
            | kind::XML_ENTITY_LITERAL_TOKEN
    )
}

/// The text tokens of an `XmlText` (or CDATA) node.
pub fn text_tokens(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| is_text_token(t.kind()))
        .collect()
}

/// StyleCop's `IsConsideredEmpty`: text is blank, an element is blank when its content is, an
/// empty element (`<a/>`) is not blank unless `empty_is_blank`.
pub fn is_blank(node: &SyntaxNode, empty_is_blank: bool) -> bool {
    match node.kind() {
        kind::XML_TEXT | kind::XML_C_DATA_SECTION => {
            text_tokens(node).iter().all(|t| t.text().trim().is_empty())
        }
        kind::XML_ELEMENT => content(node).all(|c| is_blank(&c, false)),
        kind::XML_EMPTY_ELEMENT => empty_is_blank,
        kind::XML_PROCESSING_INSTRUCTION => false,
        _ => true,
    }
}

/// The nodes between the start and the end tag.
pub fn content(element: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    element.children().filter(|c| {
        !matches!(
            c.kind(),
            kind::XML_ELEMENT_START_TAG | kind::XML_ELEMENT_END_TAG
        )
    })
}

fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !space {
                out.push(' ');
            }
            space = true;
        } else {
            out.push(ch);
            space = false;
        }
    }
    out
}

/// The text of a text node with runs of white space collapsed to one space.
pub fn normalized_text(node: &SyntaxNode) -> String {
    collapse(
        &text_tokens(node)
            .iter()
            .map(|t| t.text())
            .collect::<String>(),
    )
}

/// StyleCop's `GetText(node, true)`: text of text nodes and of the content of elements, an
/// empty element as its tag; `None` for other nodes.
pub fn xml_text(node: &SyntaxNode) -> Option<String> {
    match node.kind() {
        kind::XML_TEXT => Some(normalized_text(node)),
        kind::XML_ELEMENT => Some(content(node).filter_map(|c| xml_text(&c)).collect()),
        kind::XML_EMPTY_ELEMENT => {
            let element = Element::new(node.clone());
            let mut tag = format!("<{}", element.name);
            for attr in element.attrs() {
                let name = attr_name(&attr);
                tag.push_str(&format!(" {name}=\"{}\"", attribute_value(&attr)));
            }
            tag.push_str("/>");
            Some(tag)
        }
        _ => None,
    }
}

/// The first text node with content in `node`, looking into elements but not empty ones.
pub fn first_text(node: &SyntaxNode) -> Option<SyntaxNode> {
    match node.kind() {
        kind::XML_TEXT => (!is_blank(node, false)).then(|| node.clone()),
        kind::XML_ELEMENT => content(node).find_map(|c| first_text(&c)),
        _ => None,
    }
}

fn attr_name(attr: &SyntaxNode) -> String {
    attr.children()
        .find(|c| c.kind() == kind::XML_NAME)
        .map(|n| name_of(&n))
        .unwrap_or_default()
}

fn attribute_value(attr: &SyntaxNode) -> String {
    let text = attr.text().to_string();
    let after = text.split_once('=').map_or("", |(_, v)| v).trim();
    after.trim_matches(['"', '\'']).trim().to_string()
}

impl Element {
    pub fn new(node: SyntaxNode) -> Element {
        Element {
            name: element_name(&node),
            node,
        }
    }

    pub fn is_empty_element(&self) -> bool {
        self.node.kind() == kind::XML_EMPTY_ELEMENT
    }

    pub fn range(&self) -> TextRange {
        self.node.text_range()
    }

    /// Does the element have no text? An empty element (`<a/>`) counts when `empty_is_blank`.
    pub fn is_blank(&self, empty_is_blank: bool) -> bool {
        is_blank(&self.node, empty_is_blank)
    }

    fn attrs(&self) -> Vec<SyntaxNode> {
        let holder = if self.node.kind() == kind::XML_ELEMENT {
            self.node
                .children()
                .find(|c| c.kind() == kind::XML_ELEMENT_START_TAG)
        } else {
            Some(self.node.clone())
        };
        holder
            .map(|h| {
                h.children()
                    .filter(|c| {
                        matches!(
                            c.kind(),
                            kind::XML_TEXT_ATTRIBUTE
                                | kind::XML_CREF_ATTRIBUTE
                                | kind::XML_NAME_ATTRIBUTE
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The first attribute called `name`.
    pub fn attr(&self, name: &str) -> Option<Attr> {
        self.attrs()
            .into_iter()
            .find(|a| attr_name(a) == name)
            .map(|a| Attr {
                kind: a.kind(),
                value: attribute_value(&a),
                range: a.text_range(),
            })
    }

    /// The value of a `name="..."` attribute of the kind Roslyn gives it on `param` and
    /// `typeparam`; `None` when missing or blank.
    pub fn declared_name(&self) -> Option<Attr> {
        self.attr("name")
            .filter(|a| a.kind == kind::XML_NAME_ATTRIBUTE && !a.value.trim().is_empty())
    }

    /// The nodes between the tags (none for an empty element).
    pub fn content(&self) -> Vec<SyntaxNode> {
        if self.node.kind() == kind::XML_ELEMENT {
            content(&self.node).collect()
        } else {
            Vec::new()
        }
    }
}

impl Declaration {
    pub fn has_modifier(&self, modifier: SyntaxKind) -> bool {
        has_modifier(&self.node, modifier)
    }

    pub fn is_partial(&self) -> bool {
        self.has_modifier(kind::PARTIAL_KEYWORD)
    }

    /// A partial type or method: the summary rules for partial elements apply, not the others.
    pub fn is_partial_element(&self) -> bool {
        self.is_partial() && (self.kind.is_type() || self.kind == Decl::Method)
    }

    /// Does StyleCop's default configuration ask for documentation here (`NeedsComment`)?
    pub fn needs_comment(&self) -> bool {
        let in_interface = self
            .node
            .parent()
            .is_some_and(|p| p.kind() == kind::INTERFACE_DECLARATION);
        if self.kind == Decl::Interface
            || (in_interface && matches!(declared_access(&self.node, self.kind), Access::Public))
        {
            return true;
        }
        effective_access(&self.node, self.kind) > Access::Private
    }

    /// The identifier tokens a diagnostic about the declaration points at: the name, `this` of
    /// an indexer, each variable of a field.
    pub fn names(&self) -> Vec<TextRange> {
        let direct = |k: SyntaxKind| {
            self.node
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| t.kind() == k)
                .map(|t| vec![t.text_range()])
        };
        match self.kind {
            Decl::Indexer => direct(kind::THIS_KEYWORD),
            Decl::Field | Decl::EventField => self
                .node
                .children()
                .find(|c| c.kind() == kind::VARIABLE_DECLARATION)
                .map(|d| {
                    d.children()
                        .filter(|c| c.kind() == kind::VARIABLE_DECLARATOR)
                        .filter_map(|v| {
                            v.children_with_tokens()
                                .filter_map(|e| e.into_token())
                                .find(|t| t.kind() == kind::IDENTIFIER_TOKEN)
                        })
                        .map(|t| t.text_range())
                        .collect()
                }),
            _ => direct(kind::IDENTIFIER_TOKEN),
        }
        .unwrap_or_default()
    }

    /// The return type of a method or delegate.
    pub fn return_type(&self, grammar: &Grammar) -> Option<SyntaxNode> {
        self.node
            .children()
            .find(|c| grammar.has_tag(c.kind(), "type"))
    }

    /// The type parameters of a type, method or delegate: identifier tokens.
    pub fn type_parameters(&self) -> Vec<SyntaxToken> {
        self.node
            .children()
            .find(|c| c.kind() == kind::TYPE_PARAMETER_LIST)
            .map(|list| {
                list.children()
                    .filter(|c| c.kind() == kind::TYPE_PARAMETER)
                    .filter_map(|p| {
                        p.children_with_tokens()
                            .filter_map(|e| e.into_token())
                            .find(|t| t.kind() == kind::IDENTIFIER_TOKEN)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Is the return type `void`?
    pub fn returns_void(&self, grammar: &Grammar) -> bool {
        self.return_type(grammar).is_some_and(|t| {
            t.kind() == kind::PREDEFINED_TYPE
                && t.children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .any(|k| k.kind() == kind::VOID_KEYWORD)
        })
    }

    /// The accessors of a property: `(getter, setter)` with their accessibility modifiers.
    pub fn accessors(&self) -> (Option<SyntaxNode>, Option<SyntaxNode>) {
        let list = self
            .node
            .children()
            .find(|c| c.kind() == kind::ACCESSOR_LIST);
        let find = |k: SyntaxKind| {
            list.as_ref()
                .and_then(|l| l.children().find(|c| c.kind() == k))
        };
        (
            find(kind::GET_ACCESSOR_DECLARATION),
            find(kind::SET_ACCESSOR_DECLARATION),
        )
    }

    /// The effective accessibility of the declaration.
    pub fn effective_access(&self) -> Access {
        effective_access(&self.node, self.kind)
    }

    /// The effective accessibility of an accessor of the declaration.
    pub fn accessor_access(&self, accessor: &SyntaxNode) -> Access {
        let declared =
            modifier_access(accessor).unwrap_or_else(|| declared_access(&self.node, self.kind));
        if declared <= Access::Private {
            declared
        } else {
            combine(declared, self.effective_access())
        }
    }
}
