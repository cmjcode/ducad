//! Integrasi: spawn `ducad-mcp`, kirim 3 baris, baca 3 balasan JSON.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn three_requests_three_json_replies() {
    let dir = std::env::temp_dir().join(format!("ducad-mcp-stdio-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ducad-mcp"))
        .args(["--root", dir.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
        )
        .unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
        )
        .unwrap();
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#).unwrap();
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"new_part","arguments":{{}}}}}}"#).unwrap();
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let lines: Vec<String> = BufReader::new(&out.stdout[..])
        .lines()
        .map(Result::unwrap)
        .collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    for (i, l) in lines.iter().enumerate() {
        let v: serde_json::Value =
            serde_json::from_str(l).unwrap_or_else(|e| panic!("baris {i} bukan JSON: {e}"));
        assert_eq!(v["id"], i as u64 + 1);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
