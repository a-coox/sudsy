//! Test double for the worker protocol, driven by the proxy contract tests.
//!
//! Usage: fake_worker <mode> [marker file]
//! Modes: echo (healthy), refuse (version_mismatch error), bad-hash (replies a different grammar
//! hash), crash-on-parse, crash-once (crashes on the first Parse across restarts, using the marker
//! file), hang-on-parse.

use core_syntax::LanguageId;
use core_syntax::SyntaxKind;
use core_syntax::wire::FlatTree;
use provider_api::protocol::{
    Empty, Hello, Parsed, ProtocolError, Request, Response, decode, read_frame, send,
};
use std::io::{stdin, stdout};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("echo");
    let marker = args.get(2);
    let (mut input, mut output) = (stdin().lock(), stdout().lock());
    while let Ok(Some(frame)) = read_frame(&mut input) {
        let reply = match decode::<Request>(&frame).expect("valid request") {
            Request::Hello(h) => match mode {
                "refuse" => Response::Error(ProtocolError {
                    code: "version_mismatch".into(),
                    message: "fake refusal".into(),
                }),
                "bad-hash" => Response::Hello(Hello {
                    grammar_hash: "other".into(),
                    ..h
                }),
                _ => Response::Hello(h),
            },
            Request::Parse(p) => {
                match mode {
                    "crash-on-parse" => std::process::exit(1),
                    "crash-once" => {
                        let marker = marker.expect("marker path");
                        if !std::path::Path::new(marker).exists() {
                            std::fs::write(marker, "x").unwrap();
                            std::process::exit(1);
                        }
                    }
                    "hang-on-parse" => std::thread::sleep(std::time::Duration::from_secs(3600)),
                    _ => {}
                }
                // One root node holding one token with the whole text.
                let root = SyntaxKind::new(LanguageId::CSHARP, false, 0);
                let tok = SyntaxKind::new(LanguageId::CSHARP, true, 0);
                let flat = FlatTree {
                    kinds: vec![root.0, tok.0],
                    values: vec![1, p.text.len() as u32],
                };
                Response::Parsed(Parsed {
                    file_id: p.file_id,
                    version: p.version,
                    tree: flat.to_wire(),
                    errors: vec![],
                })
            }
            Request::Shutdown(_) => {
                send(&mut output, &Response::Ok(Empty {})).unwrap();
                return;
            }
            _ => Response::Error(ProtocolError {
                code: "unsupported".into(),
                message: "no".into(),
            }),
        };
        send(&mut output, &reply).unwrap();
    }
}
