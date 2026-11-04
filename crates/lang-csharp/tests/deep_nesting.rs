//! Deeply nested code formats on the default test thread stack (2 MB, debug frames), and nesting
//! beyond what a tree may have is an error, not a stack overflow. Each shape below overflowed
//! before the layout recursion and the document drop grew their stack on demand.

mod common;

use common::{fmt, subject};
use core_format::testing::Subject;

fn method(body: &str) -> String {
    format!("class C {{ void M() {{ {body} }} }}\n")
}

#[test]
fn deep_shapes_format() {
    let shapes = [
        format!("var x = {}1{};", "f(".repeat(150), ")".repeat(150)),
        format!("{}{}", "{".repeat(300), "}".repeat(300)),
        format!("if (a) b(); {}", "else if (a) b();".repeat(250)),
        format!("var x = a{};", ".B()".repeat(800)),
    ];
    for body in shapes {
        fmt(&method(&body), &[]);
    }
}

#[test]
fn nesting_beyond_the_tree_depth_limit_is_an_error() {
    let body = format!("var x = {}1{};", "f(".repeat(1500), ")".repeat(1500));
    let err = subject(&[]).format(&method(&body)).unwrap_err();
    assert!(err.contains("deeper than"), "{err}");
}
