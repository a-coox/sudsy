//! Option interactions: for every pair of options in the schema and every pair of sample values
//! (the others at their defaults), formatting must stay idempotent and keep the meaning.
//! The combinations are generated from `option_schema()`, so a new option is covered at once.

use crate::common::subject;
use core_config::OptionValue;
use core_format::testing::check;
use lang_json::option_schema;

/// Valid plain JSON.
const PLAIN: &[&str] = &[
    "{\"name\":\"sudsy\",\"tags\":[\"a\",\"b\"],\"nested\":{\"x\":1,\"y\":[1,2,3]},\"empty\":{},\"none\":[]}",
    "{\n  \"a\": [\n    1, 2, 3],\n\n  \"b\": {\"c\": [[1, 2], [3, 4]],\n \"d\": null}\n}\n",
    "[\"alpha\", \"beta\", \"gamma\", \"delta\", \"epsilon\", \"zeta\", \"eta\", \"theta\", \"iota\", \"kappa\", \"lambda\"]",
    "[[], {}, [[]], [{}], {\"a\": {}}]",
];

/// Needs the JSONC dialect.
const COMMENTED: &[&str] = &[
    "// header\n{\n  // a\n  \"a\": 1, // one\n  \"b\": [1, /* two */ 2,\n  ],\n  /* dangling */\n}\n// footer\n",
    "{\"a\": [ // first\n 1, 2], \"b\": {\n // only a comment\n}}",
    "[1, 2 // two\n, 3,]",
];

fn pairs(defs: &[(&'static str, Vec<OptionValue>)]) -> Vec<Vec<(&'static str, OptionValue)>> {
    let mut out = Vec::new();
    for (i, (a, a_values)) in defs.iter().enumerate() {
        for (b, b_values) in &defs[i + 1..] {
            for av in a_values {
                for bv in b_values {
                    out.push(vec![(*a, av.clone()), (*b, bv.clone())]);
                }
            }
        }
    }
    out
}

#[test]
fn every_pair_of_option_values_is_idempotent_and_keeps_the_meaning() {
    let schema = option_schema();
    let defs: Vec<_> = schema
        .defs()
        .iter()
        .map(|d| (d.name, d.sample_values()))
        .collect();
    let combos = pairs(&defs);
    assert!(combos.len() > 50, "{} combinations", combos.len());
    let mut runs = 0;
    for options in combos {
        let jsonc = options
            .iter()
            .all(|(k, v)| *k != "dialect" || *v != OptionValue::String("json".into()));
        for (path, inputs) in [("a.json", PLAIN), ("a.jsonc", COMMENTED)] {
            if path == "a.jsonc" && !jsonc {
                continue;
            }
            let s = subject(path, &options);
            for input in inputs {
                if let Err(e) = check(&s, input) {
                    panic!("options {options:?} on {path}: {e}\n--- input\n{input}");
                }
                runs += 1;
            }
        }
    }
    println!("{runs} formatting runs");
}

#[test]
fn comments_are_rejected_by_the_json_dialect() {
    let s = subject("a.json", &[]);
    assert!(
        check(&s, COMMENTED[0])
            .unwrap_err()
            .contains("comments are not allowed")
    );
}
