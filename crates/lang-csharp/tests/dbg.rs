mod common;
use core_format::{DirectiveTable, FormatCtx, format_tree};
use provider_api::{LanguageProvider, SourceFile};

#[test]
fn dbg() {
    let Ok(path) = std::env::var("DBG_FILE") else { return };
    let text = std::fs::read_to_string(path).unwrap();
    let p = common::shared_provider();
    let mut f = SourceFile::new("a.cs", 1, &text);
    f.preprocessor_symbols = common::symbols_for(std::path::Path::new(&std::env::var("DBG_FILE").unwrap()));
    let r = p.parse(&f).unwrap();
    let opts = lang_csharp::option_schema().defaults();
    let d = DirectiveTable::default();
    let cx = FormatCtx::new(p.grammar(), &opts, &d, p.formatter());
    println!("=====\n{}=====", format_tree(&r.tree, &cx).unwrap());
}
