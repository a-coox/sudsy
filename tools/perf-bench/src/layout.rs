//! `perf-bench layout LANG [--inputs DIR]`: what the layout rules cost the `format` query. The
//! same files are formatted through the incremental engine with no layout rule and with 1, 10 and
//! 100 pattern layout rules whose patterns rarely match (the matcher's cost, not the directives').

use crate::extensions;
use crate::fixed::{self, make_provider};
use anyhow::Result;
use std::path::Path;
use std::time::Instant;

fn rules(lang: &str, n: usize) -> String {
    (0..n)
        .map(|i| match lang {
            "json" => format!(
                "id: bench-layout-{i}\nlanguage: json\nlayout: [break-all]\nrule:\n  pattern: '\"k{i}\": true'\n---\n"
            ),
            "yaml" => format!(
                "id: bench-layout-{i}\nlanguage: yaml\nlayout: [break-all]\nrule:\n  pattern: 'k{i}: true'\n---\n"
            ),
            _ => format!(
                "id: bench-layout-{i}\nlanguage: csharp\nlayout: [break-all]\nrule:\n  pattern: $X.M{i}($$$A)\n---\n"
            ),
        })
        .collect::<String>()
        .trim_end_matches("---\n")
        .to_string()
}

pub fn run(lang: &str, dir: &Path) -> Result<()> {
    let files = fixed::texts(lang, dir)?;
    let bytes: usize = files.iter().map(|f| f.1.len()).sum();
    println!(
        "{lang}: {} files, {bytes} bytes; format query, best of 5",
        files.len()
    );
    for n in [0usize, 1, 10, 100] {
        let source = rules(lang, n);
        let mut best = f64::MAX;
        for _ in 0..5 {
            let provider = make_provider(lang)?;
            let loaded = if n == 0 {
                None
            } else {
                Some(extensions::load_rules(provider.as_ref(), &source)?)
            };
            let mut db = core_incremental::Database::new(vec![make_provider(lang)?]);
            if let Some(loaded) = loaded {
                db.set_extensions(loaded);
            }
            let inputs: Vec<_> = files
                .iter()
                .map(|(p, t)| {
                    core_incremental::File::new(
                        &db,
                        std::path::PathBuf::from(p),
                        1,
                        t.clone(),
                        false,
                        core_config::EditorConfig::default(),
                    )
                })
                .collect();
            let start = Instant::now();
            let ok = inputs
                .iter()
                .filter(|f| core_incremental::format(&db, **f).is_ok())
                .count();
            best = best.min(start.elapsed().as_secs_f64() * 1e3);
            assert_eq!(ok, inputs.len(), "every file formats");
        }
        println!("  {n:>3} layout rules: {best:8.1} ms");
    }
    Ok(())
}
