//! `sudsy lsp` speaks the protocol on stdin and stdout and exits when asked.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{body}", body.len())
}

fn read_message(out: &mut impl BufRead) -> String {
    let mut length = 0;
    loop {
        let mut line = String::new();
        out.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some(n) = line.strip_prefix("Content-Length: ") {
            length = n.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    out.read_exact(&mut body).unwrap();
    String::from_utf8(body).unwrap()
}

#[test]
fn lsp_command_answers_initialize_and_exits_after_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("lsp")
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let send = |stdin: &mut std::process::ChildStdin, body: &str| {
        stdin.write_all(frame(body).as_bytes()).unwrap();
        stdin.flush().unwrap();
    };

    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#,
    );
    let reply = read_message(&mut stdout);
    assert!(reply.contains(r#""name":"sudsy""#), "{reply}");
    assert!(reply.contains("diagnosticProvider"), "{reply}");
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#,
    );
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#,
    );
    let reply = read_message(&mut stdout);
    assert!(reply.contains(r#""id":2"#), "{reply}");
    send(&mut stdin, r#"{"jsonrpc":"2.0","method":"exit"}"#);
    assert!(child.wait().unwrap().success());
}
