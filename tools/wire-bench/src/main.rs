//! Usage: wire-bench <bench dir written by Sudsy.Bench>
//!
//! For each candidate encoding, times (1) deserializing to a `FlatTree` and (2) building the
//! rowan green tree from it (common to all options), and checks every tree prints back to its source.

use anyhow::{Context, Result, ensure};
use core_syntax::SyntaxNode;
use core_syntax::wire::{FlatTree, WireTree};
use prost::Message;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Message)]
struct Proto {
    #[prost(uint32, repeated, tag = "1")]
    kinds: Vec<u32>,
    #[prost(uint32, repeated, tag = "2")]
    values: Vec<u32>,
}

fn flat_arrays(b: &[u8]) -> Result<FlatTree> {
    ensure!(&b[..4] == b"SUDS", "bad magic");
    let kinds_len = u32::from_le_bytes(b[8..12].try_into()?) as usize * 2;
    let values_at = 12 + kinds_len;
    let values_len = u32::from_le_bytes(b[values_at..values_at + 4].try_into()?) as usize;
    let wire = WireTree {
        version: 1,
        kinds: b[12..values_at].to_vec(),
        values: b[values_at + 4..values_at + 4 + values_len].to_vec(),
    };
    Ok(wire.unpack()?)
}

fn msgpack_per_element(b: &[u8]) -> Result<FlatTree> {
    let elements: Vec<(u16, u32)> = rmp_serde::from_slice(b)?;
    Ok(FlatTree {
        kinds: elements.iter().map(|e| e.0).collect(),
        values: elements.iter().map(|e| e.1).collect(),
    })
}

fn protobuf_packed(b: &[u8]) -> Result<FlatTree> {
    let p = Proto::decode(b)?;
    Ok(FlatTree {
        kinds: p.kinds.iter().map(|&k| k as u16).collect(),
        values: p.values,
    })
}

fn msgpack_bin_fields(b: &[u8]) -> Result<FlatTree> {
    let wire: WireTree = rmp_serde::from_slice(b)?;
    Ok(wire.unpack()?)
}

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
    type Decoder = fn(&[u8]) -> Result<FlatTree>;
    let options: [(&str, Decoder); 4] = [
        ("a", flat_arrays),
        ("b", msgpack_per_element),
        ("c", protobuf_packed),
        ("d", msgpack_bin_fields),
    ];
    for (name, decode) in options {
        let blobs = read_container(dir.join(format!("opt-{name}.bin")))?;
        ensure!(blobs.len() == texts.len(), "file count mismatch");
        let (mut best_de, mut best_build) = (Duration::MAX, Duration::MAX);
        for pass in 0..4 {
            let (mut de, mut build) = (Duration::ZERO, Duration::ZERO);
            for (blob, text) in blobs.iter().zip(&texts) {
                let t = Instant::now();
                let flat = decode(blob)?;
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
            "option {name}: deserialize {:.0} ms, build tree {:.0} ms",
            best_de.as_secs_f64() * 1e3,
            best_build.as_secs_f64() * 1e3
        );
    }
    Ok(())
}
