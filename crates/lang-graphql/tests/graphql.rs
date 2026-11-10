use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_graphql::provider(), "a.graphql", input)
}

/// Every `.graphql` file of the pinned corpus (`tests/conformance/fetch-graphql-corpus.sh`)
/// parses, formats with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-graphql --test graphql -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (_, unparsed) = check_corpus(
        lang_graphql::provider(),
        "a.graphql",
        "graphql",
        &["graphql", "gql"],
    );
    assert!(unparsed.is_empty(), "valid files with syntax errors");
}

#[test]
fn prettier_ignore_keeps_the_next_definition() {
    assert_eq!(
        fmt("# prettier-ignore
type A{a:Int}
type B{b:Int}
"),
        "# prettier-ignore
type A{a:Int}
type B {
  b: Int
}
"
    );
}

#[test]
fn operations_and_selections() {
    assert_eq!(
        fmt(
            "query Q($a:Int=1,$b:[String!]!)@x{a,b:c(x:1,y:\"s\"){d}\n\n\n...F ...on T@d{e}}\n{ z }\nfragment F on T{f}\n"
        ),
        "query Q($a: Int = 1, $b: [String!]!) @x {\n  a\n  b: c(x: 1, y: \"s\") {\n    d\n  }\n\n  ...F\n  ... on T @d {\n    e\n  }\n}\n{\n  z\n}\nfragment F on T {\n  f\n}\n"
    );
}

#[test]
fn long_lists_break_without_commas() {
    assert_eq!(
        fmt(
            "{ f(aaaaaaaaaaaaaaaa: 1, bbbbbbbbbbbbbbbbbbbb: [1, 2], cccccccccccccccccc: {x: 1, y: 2}) }\n"
        ),
        "{\n  f(\n    aaaaaaaaaaaaaaaa: 1\n    bbbbbbbbbbbbbbbbbbbb: [1, 2]\n    cccccccccccccccccc: { x: 1, y: 2 }\n  )\n}\n"
    );
}

#[test]
fn schema_definitions() {
    assert_eq!(
        fmt(
            "\"\"\"\n  Doc\n   more\n\"\"\"\ntype A implements B&C@d{\"f\"f(a:Int=1):[A]!@x\n g:Int}\nunion U=|A|B\nenum E{X Y@d}\ninput I{a:Int=1}\nscalar S@a\ndirective @d(a:Int)repeatable on|FIELD|OBJECT\nschema{query:Q}\nextend type A@x\n"
        ),
        "\"\"\"\nDoc\n more\n\"\"\"\ntype A implements B & C @d {\n  \"f\"\n  f(a: Int = 1): [A]! @x\n  g: Int\n}\nunion U = A | B\nenum E {\n  X\n  Y @d\n}\ninput I {\n  a: Int = 1\n}\nscalar S @a\ndirective @d(a: Int) repeatable on FIELD | OBJECT\nschema {\n  query: Q\n}\nextend type A @x\n"
    );
}

#[test]
fn long_unions_break() {
    assert_eq!(
        fmt(
            "union SearchResult = AaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaA | BbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbB | C\n"
        ),
        "union SearchResult =\n  | AaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaA\n  | BbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbB\n  | C\n"
    );
}

#[test]
fn comments_are_kept() {
    let src =
        "# top\nquery Q {\n  # in\n  a # x\n  b(\n    # arg\n    x: 1\n  )\n  # end\n}\n# tail\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn dangling_comments_and_long_interfaces() {
    assert_eq!(
        fmt(
            "{ f(a: {\n# c\n}) }\ntype Thing implements Aaaaaaaaaaaaaaaaaaaa & Bbbbbbbbbbbbbbbbbbbb & Cccccccccccccccccccc { a: Int }\n"
        ),
        "{\n  f(\n    a: {\n      # c\n    }\n  )\n}\ntype Thing implements Aaaaaaaaaaaaaaaaaaaa &\n  Bbbbbbbbbbbbbbbbbbbb &\n  Cccccccccccccccccccc {\n  a: Int\n}\n"
    );
}

#[test]
fn block_strings_are_reindented() {
    assert_eq!(
        fmt(
            "type A {\n  \"\"\"\n      One\n        two\n\n  \"\"\"\n  a: Int\n  \"\"\"  x  \"\"\"\n  b: Int\n}\n"
        ),
        "type A {\n  \"\"\"\n  One\n    two\n  \"\"\"\n  a: Int\n  \"\"\"\n  x\n  \"\"\"\n  b: Int\n}\n"
    );
}

#[test]
fn forked_grammar_syntax() {
    assert_eq!(
        fmt(
            "\"d\" fragment F($a: Int) on T { f @a @b(x: \"q\\\"\") ...G(v: $a) }\nextend schema @a\nextend schema { query: Q mutation: M }\ndirective @d @e on FIELD\n\"d\" query { a(b: \"\"\"x \\\"\"\" y\"\"\") }\n"
        ),
        "\"d\"\nfragment F($a: Int) on T {\n  f @a @b(x: \"q\\\"\")\n  ...G(v: $a)\n}\nextend schema @a\nextend schema {\n  query: Q\n  mutation: M\n}\ndirective @d @e on FIELD\n\"d\"\nquery {\n  a(\n    b: \"\"\"\n    x \\\"\"\" y\n    \"\"\"\n  )\n}\n"
    );
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_graphql::provider(),
        &["a/x.graphql", "x.gql"],
        &["graphql", "a.graphqlx"],
    );
}
