use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_protobuf::provider(), "a.proto", input)
}

/// Every `.proto` file of the pinned corpus (`tests/conformance/fetch-proto-corpus.sh`) parses,
/// formats with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-protobuf --test protobuf -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (_, unparsed) = check_corpus(lang_protobuf::provider(), "a.proto", "proto", &["proto"]);
    assert!(unparsed.is_empty(), "valid files with syntax errors");
}

#[test]
fn statements_blocks_and_blank_lines() {
    assert_eq!(
        fmt(
            "syntax='proto3';\npackage   a.b;\nimport \"x.proto\";\noption java_package=\"x\";\n\n\n\nmessage Foo{\n  string name=1;\n  repeated   int32 xs=2 [deprecated=true];\n  map<string,   Bar> m=4;\n  oneof o { int32 a=5; }\n  reserved 1,2 to 4, 9 to max;\n  enum E{A=0; B=-1;}\n  message Inner{}\n}\nservice S{rpc M(stream A)returns(B);rpc N(A)returns(B){option deprecated=true;}}\n"
        ),
        "syntax = 'proto3';\npackage a.b;\n\nimport \"x.proto\";\n\noption java_package = \"x\";\n\nmessage Foo {\n  string name = 1;\n  repeated int32 xs = 2 [deprecated = true];\n  map<string, Bar> m = 4;\n  oneof o {\n    int32 a = 5;\n  }\n  reserved 1, 2 to 4, 9 to max;\n  enum E {\n    A = 0;\n    B = -1;\n  }\n  message Inner {}\n}\nservice S {\n  rpc M(stream A) returns (B);\n  rpc N(A) returns (B) {\n    option deprecated = true;\n  }\n}\n"
    );
    assert_eq!(fmt(""), "");
}

#[test]
fn compact_options_break_from_two() {
    assert_eq!(
        fmt(
            "message M { string a = 1 [deprecated=true, json_name=\"x\"]; string b = 2[(a).b = 1]; }\n"
        ),
        "message M {\n  string a = 1 [\n    deprecated = true,\n    json_name = \"x\"\n  ];\n  string b = 2 [(a).b = 1];\n}\n"
    );
}

#[test]
fn a_comment_in_compact_options_breaks_them() {
    assert_eq!(
        fmt("message M { int32 a = 1 [ /*c*/ x = 1]; }\n"),
        "message M {\n  int32 a = 1 [\n    /*c*/\n    x = 1\n  ];\n}\n"
    );
}

#[test]
fn message_literals_and_strings() {
    assert_eq!(
        fmt("option (a) = { x: 1 y [1,2] z { w: \"s\" } };\noption (b) = \"p\"\n  \"q\";\n"),
        "option (a) = {\n  x: 1\n  y: [\n    1,\n    2\n  ]\n  z: {w: \"s\"}\n};\noption (b) =\n  \"p\"\n  \"q\";\n"
    );
}

#[test]
fn comments_are_kept() {
    let src = "// top\nsyntax = \"proto3\"; // t\n\n/* b */\nmessage M {\n  // in\n  int32 a = 1; // x\n  // end\n}\n// tail\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_protobuf::provider(),
        &["a/x.proto", "x.proto"],
        &["proto", "a.protox"],
    );
}
