use core_format::{DirectiveTable, FormatCtx, comments};
use core_syntax::SyntaxElement;
use provider_api::{LanguageProvider, SourceFile};
use std::time::Instant;

pub fn run(p: &dyn LanguageProvider, name: &str, text: &str) {
    let f = SourceFile::new(name, 1, text);
    let parsed = p.parse(&f).unwrap();
    let root = parsed.tree;
    let config = core_config::Config::default();
    let opts = config
        .format_options(&p.grammar().language, p.option_schema())
        .unwrap();
    let d = DirectiveTable::default();
    let cx = FormatCtx::new(p.grammar(), &opts, &d, p.formatter());
    let g = p.grammar();
    macro_rules! t {
        ($label:expr, $e:expr) => {{
            let s = Instant::now();
            let r = $e;
            println!("{:<28} {:>8.1} ms", $label, s.elapsed().as_secs_f64() * 1000.0);
            r
        }};
    }
    let toks: Vec<_> = t!(
        "collect tokens",
        root.descendants_with_tokens()
            .filter_map(SyntaxElement::into_token)
            .collect()
    );
    let nodes: Vec<_> = t!("collect nodes", root.descendants().collect());
    println!("tokens {} nodes {}", toks.len(), nodes.len());
    t!("is_trivia all", toks.iter().filter(|x| g.is_trivia(x.kind())).count());
    t!(
        "leading_comments",
        toks.iter().map(|x| comments::leading_comments(g, x).len()).sum::<usize>()
    );
    t!(
        "trailing_comments",
        toks.iter().map(|x| comments::trailing_comments(g, x).len()).sum::<usize>()
    );
    t!(
        "cx.token",
        toks.iter().filter(|x| !g.is_trivia(x.kind())).map(|x| cx.token(x)).count()
    );
    t!("first_token", nodes.iter().map(|n| cx.first_token(n).is_some()).count());
    t!("last_token", nodes.iter().map(|n| cx.last_token(n).is_some()).count());
    t!(
        "text_range nodes",
        nodes.iter().map(|n| u32::from(n.text_range().start())).sum::<u32>()
    );
    t!(
        "children_with_tokens",
        nodes.iter().map(|n| n.children_with_tokens().count()).sum::<usize>()
    );
    t!("format_unguarded", cx.format_unguarded(&root).unwrap());
}
