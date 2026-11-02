//! Usage: wire-bench <bench dir written by Sudsy.Bench>
//!
//! Times the production decode path (`provider_api::protocol::decode` into `WireTree`, then
//! `WireTree::unpack`) and the rowan tree build, and checks every tree prints back to its source.

use anyhow::{Context, Result, ensure};
use core_syntax::SyntaxNode;
use core_syntax::wire::WireTree;
use provider_api::protocol;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn read_container(path: PathBuf) -> Result<Vec<Vec<u8>>> {
    let data = std::fs::read(&path).with_context(|| path.display().to_string())?;
    let mut out = Vec::new();
    let mut rest = &data[..];
    while !rest.is_empty() {
        let n = u32::from_le_bytes(rest[..4].try_into()?) as usize;
        out.push(rest[4..4 + n].to_vec());
        rest = &rest[4 + n..];
    }
    Ok(out)
}

fn main() -> Result<()> {
    let dir = PathBuf::from(std::env::args().nth(1).context("bench dir argument")?);
    let files = std::fs::read_to_string(dir.join("files.txt"))?;
    let texts: Vec<String> = files
        .lines()
        .map(|p| Ok(String::from_utf8(std::fs::read(p)?)?))
        .collect::<Result<_>>()?;
    let blobs = read_container(dir.join("trees.bin"))?;
    ensure!(blobs.len() == texts.len(), "file count mismatch");
    let (mut best_de, mut best_build) = (Duration::MAX, Duration::MAX);
    for pass in 0..4 {
        let (mut de, mut build) = (Duration::ZERO, Duration::ZERO);
        for (blob, text) in blobs.iter().zip(&texts) {
            let t = Instant::now();
            let flat = protocol::decode::<WireTree>(blob)?.unpack()?;
            de += t.elapsed();
            let t = Instant::now();
            let green = flat.build(text)?;
            build += t.elapsed();
            if pass == 0 {
                ensure!(
                    SyntaxNode::new_root(green).text() == text.as_str(),
                    "round trip failed"
                );
            }
        }
        best_de = best_de.min(de);
        best_build = best_build.min(build);
    }
    println!(
        "deserialize {:.0} ms, build tree {:.0} ms",
        best_de.as_secs_f64() * 1e3,
        best_build.as_secs_f64() * 1e3
    );
    Ok(())
}
