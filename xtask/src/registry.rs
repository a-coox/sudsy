//! The registration of the `lang.yaml` languages in the files that list them: the root manifest,
//! `languages`, `pack` and `scripts/pack.sh`. Each list is a generated region between
//! `<comment> generated:NAME` and `<comment> /generated` lines; the rest of the file is by hand.

use crate::lang_crate::{Lang, text};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// The file with the body of each region replaced (`(comment prefix, name, body)`).
fn splice(rel: &str, regions: &[(&str, &str, String)]) -> Result<(PathBuf, String)> {
    let mut out = std::fs::read_to_string(crate::repo_root().join(rel))
        .with_context(|| format!("reading {rel}"))?;
    for (c, name, body) in regions {
        let start = format!("{c} generated:{name}\n");
        let from = out
            .find(&start)
            .with_context(|| format!("{rel}: no `{c} generated:{name}` line"))?
            + start.len();
        let len = out[from..]
            .find(&format!("{c} /generated"))
            .with_context(|| format!("{rel}: `{name}` has no `{c} /generated`"))?;
        // The end marker's indentation stays.
        let len = out[from..from + len].rfind('\n').map_or(0, |i| i + 1);
        out.replace_range(from..from + len, body);
    }
    Ok((rel.into(), out))
}

/// One line per language.
fn lines(langs: &[Lang], f: impl Fn(&Lang) -> String) -> String {
    langs.iter().map(|l| f(l) + "\n").collect()
}

fn grammar(l: &Lang) -> &str {
    text(&l.yaml, "grammar-crate", "")
}

fn name(l: &Lang) -> &str {
    l.krate.trim_start_matches("lang-")
}

/// The grammar crate's workspace dependency value (`grammar-path`: a fork in the repository).
fn version(l: &Lang) -> String {
    if let Some(path) = l.yaml.get("grammar-path").and_then(|v| v.as_str()) {
        return format!("{{ path = {path:?} }}");
    }
    let v = text(&l.yaml, "grammar-version", "");
    match l.yaml.get("grammar-default-features").is_some() {
        true => format!("{{ version = {v:?}, default-features = false }}"),
        false => format!("{v:?}"),
    }
}

/// The `pack` exports of a language: its grammar, and the inline grammar of a composite.
fn exports(l: &Lang) -> String {
    let krate = grammar(l).replace('-', "_");
    let one =
        |konst: &str, symbol: &str| format!("    {:?}, {symbol}, {krate}::{konst};\n", name(l));
    let mut out = one(
        text(&l.yaml, "language-const", "LANGUAGE"),
        "sudsy_language",
    );
    if let Some(konst) = l.yaml.get("inline-language-const").and_then(|v| v.as_str()) {
        out += &one(konst, "sudsy_language_inline");
    }
    out
}

pub fn generate(langs: &[Lang]) -> Result<Vec<(PathBuf, String)>> {
    let features = langs
        .iter()
        .map(|l| format!("\"{}/static-grammars\"", l.krate))
        .collect::<Vec<_>>()
        .join(", ");
    let packs = langs.iter().map(name).collect::<Vec<_>>().join(" ");
    Ok(vec![
        splice(
            "Cargo.toml",
            &[
                (
                    "#",
                    "grammars",
                    lines(langs, |l| format!("{} = {}", grammar(l), version(l))),
                ),
                (
                    "#",
                    "languages",
                    lines(langs, |l| {
                        format!(
                            "{0} = {{ path = \"crates/{0}\", default-features = false }}",
                            l.krate
                        )
                    }),
                ),
            ],
        )?,
        splice(
            "crates/languages/Cargo.toml",
            &[
                (
                    "#",
                    "dependencies",
                    lines(langs, |l| format!("{}.workspace = true", l.krate)),
                ),
                (
                    "#",
                    "static-grammars",
                    format!("static-grammars = [{features}]\n"),
                ),
            ],
        )?,
        splice(
            "crates/languages/src/lib.rs",
            &[(
                "//",
                "providers",
                lines(langs, |l| {
                    format!(
                        "        Box::new({}::provider()),",
                        l.krate.replace('-', "_")
                    )
                }),
            )],
        )?,
        splice(
            "crates/pack/Cargo.toml",
            &[
                (
                    "#",
                    "dependencies",
                    lines(langs, |l| {
                        format!("{} = {{ workspace = true, optional = true }}", grammar(l))
                    }),
                ),
                (
                    "#",
                    "features",
                    lines(langs, |l| format!("{} = [\"dep:{}\"]", name(l), grammar(l))),
                ),
            ],
        )?,
        splice(
            "crates/pack/src/lib.rs",
            &[("//", "exports", langs.iter().map(exports).collect())],
        )?,
        splice(
            "scripts/pack.sh",
            &[("#", "languages", format!("all=({packs})\n"))],
        )?,
    ])
}
