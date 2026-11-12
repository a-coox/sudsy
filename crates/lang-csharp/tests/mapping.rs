//! The hard cases of the Roslyn mapping, checked through the typed wrappers: top-level statements,
//! file-scoped namespaces, records, collection expressions, strings, structured trivia, directives,
//! error recovery, tags and field accessors.

use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, grammars, print};
use lang_csharp::generated::{self as cs, kind};
use lang_csharp::provider;
use provider_api::{LanguageProvider, ParseError, SourceFile};

fn parse_with(text: &str, symbols: &[&str]) -> (SyntaxNode, Vec<ParseError>) {
    let p = provider(&crate::common::worker_dll());
    let mut file = SourceFile::new("m.cs", 1, text);
    file.preprocessor_symbols = symbols.iter().map(|s| s.to_string()).collect();
    let result = p.parse(&file).unwrap();
    assert_eq!(print(&result.tree), text);
    (result.tree, result.errors)
}

fn parse(text: &str) -> SyntaxNode {
    let (tree, errors) = parse_with(text, &[]);
    assert!(errors.is_empty(), "{errors:?}");
    tree
}

fn first<N: AstNode>(tree: &SyntaxNode) -> N {
    tree.descendants()
        .find_map(N::cast)
        .unwrap_or_else(|| panic!("no {}", std::any::type_name::<N>()))
}

fn text(n: &impl AstNode) -> String {
    n.syntax().text().to_string().trim().to_string()
}

#[test]
fn top_level_statements_are_global_statements_of_the_compilation_unit() {
    let tree = parse("using System;\nvar x = 1;\nreturn x;\nclass C { }\n");
    let unit = cs::CompilationUnit::cast(tree).unwrap();
    assert_eq!(unit.usings().count(), 1);
    let members: Vec<_> = unit.members().collect();
    assert_eq!(members.len(), 3);
    let g = cs::GlobalStatement::cast(members[0].syntax().clone()).unwrap();
    assert!(cs::LocalDeclarationStatement::cast(g.statement().unwrap().syntax().clone()).is_some());
    let g = cs::GlobalStatement::cast(members[1].syntax().clone()).unwrap();
    assert!(cs::ReturnStatement::cast(g.statement().unwrap().syntax().clone()).is_some());
    assert!(cs::ClassDeclaration::cast(members[2].syntax().clone()).is_some());
    assert!(unit.end_of_file().is_some());
}

#[test]
fn file_scoped_namespaces_hold_the_following_members() {
    let tree = parse("namespace A.B;\n\nclass C { }\nstruct S { }\n");
    let ns: cs::FileScopedNamespaceDeclaration = first(&tree);
    assert_eq!(text(&ns.name().unwrap()), "A.B");
    assert!(ns.semicolon().is_some());
    assert_eq!(ns.members().count(), 2);
}

#[test]
fn records_have_primary_constructors_and_base_calls() {
    let tree = parse("record P(int X, string Y) : B(X) { }\nrecord struct Q(int Z);\n");
    let mut records = tree.descendants().filter_map(cs::RecordDeclaration::cast);
    let p = records.next().unwrap();
    assert_eq!(p.keyword().unwrap().text(), "record");
    assert_eq!(p.identifier().unwrap().text(), "P");
    assert_eq!(p.parameter_list().unwrap().parameters().count(), 2);
    let base = p.base_list().unwrap().types().next().unwrap();
    let base = cs::PrimaryConstructorBaseType::cast(base.syntax().clone()).unwrap();
    assert_eq!(text(&base.type_().unwrap()), "B");
    assert_eq!(base.argument_list().unwrap().arguments().count(), 1);
    assert!(p.open_brace().is_some() && p.semicolon().is_none());

    let q = tree
        .descendants()
        .find_map(cs::RecordStructDeclaration::cast)
        .expect("record struct has its own kind");
    assert_eq!(q.class_or_struct_keyword().unwrap().text(), "struct");
    assert!(q.semicolon().is_some());
}

#[test]
fn collection_expressions_have_elements_and_spreads() {
    let tree = parse("class C { int[] a = [1, ..b, 2,]; int[] e = []; }");
    let mut all = tree
        .descendants()
        .filter_map(cs::CollectionExpression::cast);
    let a = all.next().unwrap();
    let elements: Vec<_> = a.elements().collect();
    assert_eq!(elements.len(), 3);
    assert_eq!(
        elements
            .iter()
            .map(|e| e.syntax().kind())
            .collect::<Vec<_>>(),
        [
            kind::EXPRESSION_ELEMENT,
            kind::SPREAD_ELEMENT,
            kind::EXPRESSION_ELEMENT
        ]
    );
    let spread = cs::SpreadElement::cast(elements[1].syntax().clone()).unwrap();
    assert_eq!(spread.operator().unwrap().text(), "..");
    assert_eq!(text(&spread.expression().unwrap()), "b");
    assert_eq!(all.next().unwrap().elements().count(), 0);
}

#[test]
fn string_literals_keep_their_flavors() {
    let src = "class C { object[] s = { \"a\", @\"b\"\"c\", \"\"\"raw\"\"\", \"\"\"\"\n    multi\n    \"\"\"\", \"u\"u8 }; }";
    let tree = parse(src);
    let literal_tokens: Vec<_> = tree
        .descendants()
        .filter_map(cs::StringLiteralExpression::cast)
        .map(|e| e.token().unwrap().kind())
        .collect();
    assert_eq!(
        literal_tokens,
        [
            kind::STRING_LITERAL_TOKEN,
            kind::STRING_LITERAL_TOKEN,
            kind::SINGLE_LINE_RAW_STRING_LITERAL_TOKEN,
            kind::MULTI_LINE_RAW_STRING_LITERAL_TOKEN,
        ]
    );
    let u8 = first::<cs::Utf8StringLiteralExpression>(&tree);
    assert_eq!(u8.token().unwrap().kind(), kind::UTF8_STRING_LITERAL_TOKEN);
    let g = grammars::csharp();
    assert!(g.has_tag(kind::STRING_LITERAL_TOKEN, "string"));
    assert!(g.has_tag(kind::MULTI_LINE_RAW_STRING_LITERAL_TOKEN, "string"));
}

#[test]
fn interpolated_strings_split_into_text_and_interpolations() {
    let tree = parse(
        "class C { string s = $\"a {x,-5:F2} b {{c}} {y}\"; string r = $$\"\"\"{{z}}\"\"\"; }",
    );
    let mut all = tree
        .descendants()
        .filter_map(cs::InterpolatedStringExpression::cast);
    let s = all.next().unwrap();
    assert_eq!(
        s.string_start().unwrap().kind(),
        kind::INTERPOLATED_STRING_START_TOKEN
    );
    let contents: Vec<_> = s.contents().map(|c| c.syntax().kind()).collect();
    assert_eq!(
        contents
            .iter()
            .filter(|k| **k == kind::INTERPOLATION)
            .count(),
        2
    );
    assert!(contents.contains(&kind::INTERPOLATED_STRING_TEXT));
    let i = s
        .contents()
        .find_map(|c| cs::Interpolation::cast(c.syntax().clone()))
        .unwrap();
    assert_eq!(text(&i.expression().unwrap()), "x");
    assert_eq!(text(&i.alignment_clause().unwrap()), ",-5");
    assert_eq!(text(&i.format_clause().unwrap()), ":F2");
    let r = all.next().unwrap();
    assert_eq!(
        r.string_start().unwrap().kind(),
        kind::INTERPOLATED_SINGLE_LINE_RAW_STRING_START_TOKEN
    );
}

#[test]
fn doc_comments_are_structured_trivia_nodes() {
    let src = "/// <summary>A <see cref=\"X\"/> doc.</summary>\n/// <param name=\"p\">p</param>\nclass C { }\n";
    let tree = parse(src);
    let doc = first::<cs::SingleLineDocumentationCommentTrivia>(&tree);
    assert!(grammars::csharp().is_trivia(doc.syntax().kind()));
    // The doc comment is a leading trivia sibling of the `class` keyword.
    let class = first::<cs::ClassDeclaration>(&tree);
    assert_eq!(
        class.syntax().first_child().unwrap().kind(),
        doc.syntax().kind()
    );
    let elements: Vec<_> = doc
        .content()
        .filter_map(|n| cs::XmlElement::cast(n.syntax().clone()))
        .collect();
    assert_eq!(elements.len(), 2);
    let summary = &elements[0];
    assert_eq!(
        text(&summary.start_tag().unwrap().name().unwrap()),
        "summary"
    );
    let see = doc
        .syntax()
        .descendants()
        .find_map(cs::XmlEmptyElement::cast)
        .unwrap();
    let cref = see
        .attributes()
        .find_map(|a| cs::XmlCrefAttribute::cast(a.syntax().clone()))
        .unwrap();
    assert_eq!(text(&cref.name().unwrap()), "cref");
    assert_eq!(
        doc.syntax().text().to_string(),
        src[..src.rfind("class").unwrap()]
    );
}

#[test]
fn directives_are_structured_trivia_and_disabled_text_stays_verbatim() {
    let src = "#if A && !B\nclass On { }\n#else\nclass Off { }\n#endif\nclass Tail { }\n";
    let classes = |tree: &SyntaxNode| -> Vec<String> {
        tree.descendants()
            .filter_map(cs::ClassDeclaration::cast)
            .map(|c| c.identifier().unwrap().text().to_string())
            .collect()
    };
    let (tree, errors) = parse_with(src, &["A"]);
    assert!(errors.is_empty());
    let g = grammars::csharp();
    let directives: Vec<_> = tree
        .descendants()
        .filter(|n| g.has_tag(n.kind(), "directive"))
        .map(|n| n.kind())
        .collect();
    assert_eq!(
        directives,
        [
            kind::IF_DIRECTIVE_TRIVIA,
            kind::ELSE_DIRECTIVE_TRIVIA,
            kind::END_IF_DIRECTIVE_TRIVIA
        ]
    );
    let cond = first::<cs::IfDirectiveTrivia>(&tree).condition().unwrap();
    assert_eq!(cond.syntax().kind(), kind::LOGICAL_AND_EXPRESSION);
    // The symbols decide which branch is code; the other is disabled text, byte for byte.
    assert_eq!(classes(&tree), ["On", "Tail"]);
    assert_eq!(classes(&parse(src)), ["Off", "Tail"]);

    let tree = parse("#if NEVER\nnot $$$ valid { C#\n  code\n#endif\nclass Tail { }\n");
    let disabled: Vec<_> = tree
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == kind::DISABLED_TEXT_TRIVIA)
        .collect();
    assert_eq!(disabled.len(), 1);
    assert_eq!(disabled[0].text(), "not $$$ valid { C#\n  code\n");
    assert_eq!(classes(&tree), ["Tail"]);
}

#[test]
fn missing_tokens_are_zero_width_error_nodes() {
    let (tree, errors) = parse_with("class A { int x = ; }", &[]);
    assert!(!errors.is_empty());
    let missing: Vec<_> = tree
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::ERROR)
        .collect();
    assert_eq!(missing.len(), 1);
    assert!(missing[0].text_range().is_empty());
    // It stands where the identifier of the initializer should be.
    assert_eq!(missing[0].parent().unwrap().kind(), kind::IDENTIFIER_NAME);
}

#[test]
fn skipped_tokens_are_error_nodes_holding_the_tokens_and_their_trivia() {
    let path = crate::common::repo_root().join("tests/fixtures/csharp/error_skipped_tokens.cs");
    let src = std::fs::read_to_string(path).unwrap();
    let (tree, errors) = parse_with(&src, &[]);
    assert!(!errors.is_empty());
    let skipped: Vec<_> = tree
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::ERROR && !n.text_range().is_empty())
        .collect();
    assert!(!skipped.is_empty());
    // Each is a node of real tokens (with their own trivia), not one opaque trivia blob.
    let tokens: Vec<_> = skipped
        .iter()
        .flat_map(|n| n.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .collect();
    assert!(
        tokens
            .iter()
            .any(|t| t.kind() == kind::BAD_TOKEN && t.text() == "@")
    );
    // Ordinary code around the damage is still there.
    assert_eq!(
        tree.descendants()
            .filter_map(cs::ClassDeclaration::cast)
            .count(),
        2
    );
}

#[test]
fn chain_tags_mark_the_nodes_the_formatter_flattens() {
    let g = grammars::csharp();
    for k in [
        kind::ADD_EXPRESSION,
        kind::LOGICAL_AND_EXPRESSION,
        kind::LOGICAL_OR_EXPRESSION,
        kind::COALESCE_EXPRESSION,
        kind::OR_PATTERN,
        kind::AND_PATTERN,
    ] {
        assert!(g.has_tag(k, "binary-chain"), "{}", g.name(k));
    }
    for k in [
        kind::IS_EXPRESSION,
        kind::AS_EXPRESSION,
        kind::SIMPLE_ASSIGNMENT_EXPRESSION,
    ] {
        assert!(!g.has_tag(k, "binary-chain"), "{}", g.name(k));
    }
    for k in [
        kind::INVOCATION_EXPRESSION,
        kind::SIMPLE_MEMBER_ACCESS_EXPRESSION,
        kind::CONDITIONAL_ACCESS_EXPRESSION,
        kind::ELEMENT_ACCESS_EXPRESSION,
    ] {
        assert!(g.has_tag(k, "member-chain"), "{}", g.name(k));
    }
    assert!(g.has_tag(kind::SIMPLE_ASSIGNMENT_EXPRESSION, "assignment"));
    assert!(g.has_tag(kind::BLOCK, "block"));
    assert!(g.has_tag(kind::ARGUMENT_LIST, "delimited-list"));
    assert!(g.has_tag(kind::COLLECTION_EXPRESSION, "delimited-list"));
    assert!(g.has_tag(kind::COMMA_TOKEN, "separator"));
    assert!(g.has_tag(kind::NAME_COLON, "key-value"));
    assert!(g.has_tag(kind::RECORD_DECLARATION, "type-declaration"));

    let tree = parse("class C { void M() { var a = b.C(d).E; x = y + z * 2; } }");
    let add = first::<cs::AddExpression>(&tree);
    assert_eq!(text(&add.left().unwrap()), "y");
    assert_eq!(add.operator().unwrap().text(), "+");
    assert_eq!(text(&add.right().unwrap()), "z * 2");
    let access = first::<cs::SimpleMemberAccessExpression>(&tree);
    assert_eq!(text(&access.expression().unwrap()), "b.C(d)");
    assert_eq!(text(&access.name().unwrap()), "E");
}

#[test]
fn accessors_tell_apart_items_of_the_same_type() {
    let tree = parse(
        "class C { void M() { for (int i = 0, j = 1; i < j; i++, j--) { } for (;;) { } for (a = 1; ; ) { } \
         var c = p ? q : r; var d = (T)x; var e = from v in w select v; var f = from int u in w select u; } }",
    );
    let fors: Vec<_> = tree
        .descendants()
        .filter_map(cs::ForStatement::cast)
        .collect();
    assert_eq!(fors.len(), 3);
    assert_eq!(text(&fors[0].declaration().unwrap()), "int i = 0, j = 1");
    assert_eq!(fors[0].initializers().count(), 0);
    assert_eq!(text(&fors[0].condition().unwrap()), "i < j");
    assert_eq!(fors[0].incrementors().count(), 2);
    assert!(fors[1].declaration().is_none() && fors[1].condition().is_none());
    assert_eq!(fors[1].initializers().count(), 0);
    assert!(fors[2].declaration().is_none() && fors[2].condition().is_none());
    assert_eq!(text(&fors[2].initializers().next().unwrap()), "a = 1");

    let c = first::<cs::ConditionalExpression>(&tree);
    assert_eq!(text(&c.condition().unwrap()), "p");
    assert_eq!(text(&c.when_true().unwrap()), "q");
    assert_eq!(text(&c.when_false().unwrap()), "r");

    // A type name is also an expression: the cast's type must not be mistaken for its operand.
    let cast = first::<cs::CastExpression>(&tree);
    assert_eq!(text(&cast.type_().unwrap()), "T");
    assert_eq!(text(&cast.expression().unwrap()), "x");

    let mut froms = tree.descendants().filter_map(cs::FromClause::cast);
    let plain = froms.next().unwrap();
    assert!(plain.type_().is_none());
    assert_eq!(text(&plain.expression().unwrap()), "w");
    let typed = froms.next().unwrap();
    assert_eq!(text(&typed.type_().unwrap()), "int");
    assert_eq!(typed.identifier().unwrap().text(), "u");
}

#[test]
fn trivia_follows_the_shared_attachment_rule() {
    let tree = parse("class C { int a; /* t */ // c\n  /* l */ int b; }\n");
    let g = grammars::csharp();
    let class = first::<cs::ClassDeclaration>(&tree);
    let fields: Vec<_> = class.members().collect();
    assert_eq!(fields.len(), 2);
    // `a;` owns the rest of its line as trailing trivia ...
    let first_field = fields[0].syntax();
    let trailing: Vec<_> = first_field
        .children_with_tokens()
        .skip_while(|c| c.kind() != kind::SEMICOLON_TOKEN)
        .skip(1)
        .map(|c| c.kind())
        .collect();
    assert_eq!(
        trailing,
        [
            kind::WHITESPACE_TRIVIA,
            kind::MULTI_LINE_COMMENT_TRIVIA,
            kind::WHITESPACE_TRIVIA,
            kind::SINGLE_LINE_COMMENT_TRIVIA,
            kind::END_OF_LINE_TRIVIA
        ]
    );
    // ... and the next line's indentation and comment lead the next member.
    let leading: Vec<_> = fields[1]
        .syntax()
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .take(4)
        .map(|t| t.kind())
        .collect();
    assert_eq!(
        leading,
        [
            kind::WHITESPACE_TRIVIA,
            kind::MULTI_LINE_COMMENT_TRIVIA,
            kind::WHITESPACE_TRIVIA,
            kind::INT_KEYWORD
        ]
    );
    assert!(g.is_trivia(kind::MULTI_LINE_COMMENT_TRIVIA));
}
