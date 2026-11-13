//! The official yaml-test-suite (pinned in tests/conformance/fetch-yaml-corpus.sh).
//!
//! * Valid cases must parse without errors and print back to their bytes.
//! * Error cases must produce at least one error.
//!
//! Run: `bash tests/conformance/fetch-yaml-corpus.sh &&
//! cargo test --profile corpus -p lang-yaml --test integration -- --ignored --nocapture conformance::`

use core_syntax::{print, trivia};
use lang_yaml::model::{Node, Value, load};
use std::path::PathBuf;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/corpus/yaml-test-suite")
}

struct Case {
    name: String,
    dir: PathBuf,
    text: String,
    error: bool,
}

/// Every case directory: `ID` or `ID/NN`, identified by an `in.yaml` file.
fn cases() -> Vec<Case> {
    let root = corpus();
    assert!(
        root.is_dir(),
        "corpus missing, run tests/conformance/fetch-yaml-corpus.sh"
    );
    let mut out = Vec::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir() && !p.ends_with(".git"))
        .collect();
    dirs.sort();
    for d in dirs {
        let mut candidates = vec![d.clone()];
        let mut subs: Vec<PathBuf> = std::fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_dir())
            .collect();
        subs.sort();
        candidates.extend(subs);
        for c in candidates {
            if !c.join("in.yaml").is_file() {
                continue;
            }
            let rel = c
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace("\\", "/");
            let bytes = std::fs::read(c.join("in.yaml")).unwrap();
            let text = String::from_utf8(bytes).unwrap();
            let text = text
                .strip_prefix('\u{feff}')
                .map(str::to_string)
                .unwrap_or(text);
            out.push(Case {
                name: rel,
                error: c.join("error").is_file(),
                dir: c,
                text,
            });
        }
    }
    out
}

#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-yaml-corpus.sh"]
fn yaml_test_suite_parses() {
    let g = core_syntax::grammars::yaml();
    let (mut valid, mut invalid) = (0, 0);
    let (mut parse_fail, mut error_missed, mut lossy) = (Vec::new(), Vec::new(), Vec::new());
    for case in cases() {
        let parsed = lang_yaml::parse(&case.text);
        if print(&parsed.tree) != case.text {
            lossy.push(case.name.clone());
        }
        if case.error {
            invalid += 1;
            if parsed.errors.is_empty() {
                error_missed.push(case.name.clone());
            }
        } else {
            valid += 1;
            if !parsed.errors.is_empty() {
                let e = &parsed.errors[0];
                parse_fail.push(format!(
                    "{} ({}: {} at byte {})",
                    case.name, e.code, e.message, e.start
                ));
            } else if let Err(e) = trivia::check_attachment(&parsed.tree, |k| g.is_trivia(k)) {
                parse_fail.push(format!("{}: attachment: {e}", case.name));
            }
        }
    }
    println!("valid cases: {valid}, error cases: {invalid}");
    println!(
        "valid cases with errors or bad attachment: {}",
        parse_fail.len()
    );
    for f in &parse_fail {
        println!("  {f}");
    }
    println!("error cases without errors: {}", error_missed.len());
    for f in &error_missed {
        println!("  {f}");
    }
    println!("not round-tripping: {}", lossy.len());
    for f in &lossy {
        println!("  {f}");
    }
    assert!(
        parse_fail.is_empty() && error_missed.is_empty() && lossy.is_empty(),
        "parser conformance failures, see above"
    );
}

fn to_json(n: &Node) -> Result<serde_json::Value, String> {
    use serde_json::Value as J;
    Ok(match &n.value {
        Value::Null => J::Null,
        Value::Bool(b) => J::Bool(*b),
        Value::Int(i) => i64::try_from(*i)
            .map(J::from)
            .or_else(|_| u64::try_from(*i).map(J::from))
            .unwrap_or_else(|_| J::from(*i as f64)),
        Value::Float(f) => match serde_json::Number::from_f64(*f) {
            Some(n) => J::Number(n),
            None => return Err(format!("{f} has no JSON form")),
        },
        Value::Str(s) => J::String(s.clone()),
        Value::Seq(items) => J::Array(items.iter().map(to_json).collect::<Result<_, _>>()?),
        Value::Map(pairs) => {
            let mut m = serde_json::Map::new();
            for (k, v) in pairs {
                let key = match &k.value {
                    Value::Str(s) => s.clone(),
                    other => to_json(&Node {
                        tag: None,
                        value: other.clone(),
                    })?
                    .to_string(),
                };
                m.insert(key, to_json(v)?);
            }
            J::Object(m)
        }
    })
}

/// Equal, except that numbers compare by value (`450.00` and `450`).
fn json_eq(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value as J;
    match (a, b) {
        (J::Number(x), J::Number(y)) => x == y || x.as_f64() == y.as_f64(),
        (J::Array(x), J::Array(y)) => json_eq_all(x, y),
        (J::Object(x), J::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w)))
        }
        _ => a == b,
    }
}

fn json_eq_all(a: &[serde_json::Value], b: &[serde_json::Value]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| json_eq(x, y))
}

#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-yaml-corpus.sh"]
fn yaml_test_suite_data_model() {
    let (mut compared, mut matched) = (0, 0);
    let mut failures = Vec::new();
    for case in cases().into_iter().filter(|c| !c.error) {
        let Ok(json) = std::fs::read_to_string(case.dir.join("in.json")) else {
            continue;
        };
        compared += 1;
        let expected: Vec<serde_json::Value> = serde_json::Deserializer::from_str(&json)
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        let result = load(&case.text)
            .and_then(|docs| docs.iter().map(to_json).collect::<Result<Vec<_>, _>>());
        match result {
            Ok(actual) if json_eq_all(&actual, &expected) => matched += 1,
            Ok(actual) => failures.push(format!(
                "{}: expected {expected:?}, got {actual:?}",
                case.name
            )),
            Err(e) => failures.push(format!("{}: {e}", case.name)),
        }
    }
    println!("cases with in.json: {compared}, data model equal: {matched}");
    for f in &failures {
        println!("  {f}");
    }
    assert!(failures.is_empty(), "data model differences, see above");
}

#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-yaml-corpus.sh"]
fn yaml_test_suite_formats() {
    let subject = crate::common::subject(&[]);
    let (mut formatted, mut total) = (0, 0);
    let mut failures = Vec::new();
    for case in cases().into_iter().filter(|c| !c.error) {
        total += 1;
        match core_format::testing::check(&subject, &case.text) {
            Ok(_) => formatted += 1,
            Err(e) => failures.push(format!("{}: {}", case.name, e.lines().next().unwrap_or(""))),
        }
    }
    println!("valid cases formatted with every check: {formatted} of {total}");
    for f in &failures {
        println!("  {f}");
    }
    assert!(failures.is_empty(), "formatting failures, see above");
}

/// The same corpus with every option value, one option at a time and a few combinations.
#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-yaml-corpus.sh"]
fn yaml_test_suite_formats_with_every_option_value() {
    use core_config::OptionValue;
    let schema = lang_yaml::option_schema();
    let mut sets: Vec<Vec<(&str, OptionValue)>> = Vec::new();
    for def in schema.defs() {
        for v in def.sample_values() {
            sets.push(vec![(def.name, v)]);
        }
    }
    sets.push(vec![
        ("indent_size", OptionValue::Int(4)),
        ("sequence_indent", crate::common::s("indented")),
        ("quote_style", crate::common::s("single")),
        ("document_start", crate::common::s("always")),
    ]);
    sets.push(vec![
        ("indent_size", OptionValue::Int(1)),
        ("sequence_indent", crate::common::s("flush")),
        ("quote_style", crate::common::s("double")),
        ("document_start", crate::common::s("never")),
        ("line_width", OptionValue::Int(20)),
    ]);
    let cases: Vec<Case> = cases().into_iter().filter(|c| !c.error).collect();
    let mut failures = Vec::new();
    for options in &sets {
        let subject = crate::common::subject(options);
        for case in &cases {
            if let Err(e) = core_format::testing::check(&subject, &case.text) {
                failures.push(format!(
                    "{} with {options:?}: {}",
                    case.name,
                    e.lines().next().unwrap_or("")
                ));
            }
        }
    }
    println!("{} option sets x {} cases", sets.len(), cases.len());
    for f in &failures {
        println!("  {f}");
    }
    assert!(failures.is_empty(), "formatting failures, see above");
}
