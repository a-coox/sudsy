//! Option interactions: for every pair of options in the schema and every pair of sample values
//! (the others at their defaults), formatting must stay idempotent and keep the meaning.
//! The combinations are generated from `option_schema()`, so a new option is covered at once.

mod common;

use common::subject;
use core_config::OptionValue;
use core_format::testing::check;
use lang_yaml::option_schema;

const INPUTS: &[&str] = &[
    "name: sudsy\nitems:\n- one\n-   two   # trailing\n- key: value\n  other: [1,2,   3]\n  nested:\n      deep: {a: 1, b: 2}\n      list:\n        - x\n        - y\n\n      # about last\n  # about nested\n",
    "# top\nlit: |\n    keep   \n      indented\nfold: >-\n  folded\n  text\n\nkept: |+\n  a\n\nq1: 'single'\nq2: \"double \\\"x\\\"\"\nq3: \"it's\"\nq4: 'it''s'\n",
    "anchors: &a\n  k: v\nref: *a\n? complex\n: value\n? [flow, key]\n: *a\nmulti: plain\n  continued\n\"quoted\": \"two\n  lines\"\n",
    "%YAML 1.2\n---\na: 1\n...\n---\n- [alpha, beta, gamma, delta, epsilon, zeta, eta, theta, iota, kappa]\n- {k: v}\n- - nested\n  - seq\n---\n!!str scalar\n--- |\n  text\n",
    "list:\n- a\n# between\n- b\n  # inside\n- c: 1\n  d:\n  - x\n  # end of x\n# end of list\nlast: 1\n# end of file\n",
    "a:\n  - - x\n    - y\n  -\n    - z\n  - !t\n    k: v\n  - &n\n    - 1\n  - ? k\n    : v\n",
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
        let s = subject(&options);
        for input in INPUTS {
            if let Err(e) = check(&s, input) {
                panic!("options {options:?}: {e}\n--- input\n{input}");
            }
            runs += 1;
        }
    }
    println!("{runs} formatting runs");
}

#[test]
fn the_schema_declares_what_the_formatter_reads() {
    let schema = option_schema();
    let names: Vec<&str> = schema.defs().iter().map(|d| d.name).collect();
    assert_eq!(
        names,
        [
            "line_width",
            "indent_size",
            "final_newline",
            "sequence_indent",
            "quote_style",
            "document_start",
            "max_blank_lines"
        ]
    );
    // The defaults are the zero-config behavior: nothing is rewritten that need not be.
    assert_eq!(
        check(&subject(&[]), INPUTS[2]).unwrap(),
        "anchors: &a\n  k: v\nref: *a\n? complex\n: value\n? [flow, key]\n: *a\nmulti: plain\n  continued\n\"quoted\": \"two\n  lines\"\n"
    );
}
