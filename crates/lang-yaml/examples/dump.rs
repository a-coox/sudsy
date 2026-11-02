//! Debug helper: prints the tree and errors for a file (or stdin).
fn main() {
    let path = std::env::args().nth(1).expect("path");
    let text = std::fs::read_to_string(path).unwrap();
    let parsed = lang_yaml::parse(&text);
    println!(
        "{}",
        core_syntax::dump_tree(&parsed.tree, core_syntax::grammars::yaml())
    );
    for e in &parsed.errors {
        println!("error {} at {}: {}", e.code, e.start, e.message);
    }
}
