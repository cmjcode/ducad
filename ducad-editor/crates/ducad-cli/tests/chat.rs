//! Tes integrasi `ducad-cli chat` dengan server LLM palsu di localhost
//! (gaya OpenAI-compatible, tanpa kunci API): model "memanggil" `run_ops`,
//! server MCP in-process membuat body, lalu part disimpan.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::mpsc;

fn sse(chunks: &[serde_json::Value]) -> String {
    let mut s = String::new();
    for c in chunks {
        s.push_str(&format!("data: {c}\n\n"));
    }
    s.push_str("data: [DONE]\n\n");
    s
}

/// Layani `replies.len()` permintaan berurutan; kirim body permintaan ke `tx`.
fn serve(listener: TcpListener, replies: Vec<String>, tx: mpsc::Sender<serde_json::Value>) {
    for reply in replies {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut len = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let l = line.trim_end().to_ascii_lowercase();
            if l.is_empty() {
                break;
            }
            if let Some(v) = l.strip_prefix("content-length:") {
                len = v.trim().parse().unwrap();
            }
        }
        let mut body = vec![0u8; len];
        reader.read_exact(&mut body).unwrap();
        tx.send(serde_json::from_slice(&body).unwrap()).unwrap();
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            reply.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(reply.as_bytes()).unwrap();
    }
}

#[test]
fn chat_models_a_block_through_mcp_tools() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let args = serde_json::json!({ "ops": [
        { "op": "primitive", "id": "blok", "shape": { "box": { "size": [10, 20, 30] } } }
    ] })
    .to_string();
    let turn1 = sse(&[
        serde_json::json!({"choices":[{"delta":{"content":"Membuat blok."}}]}),
        serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"run_ops","arguments": args}}]}}]}),
        serde_json::json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
    ]);
    let turn2 = sse(&[
        serde_json::json!({"choices":[{"delta":{"content":"Blok 10×20×30 selesai."}}]}),
        serde_json::json!({"choices":[{"delta":{},"finish_reason":"stop"}]}),
    ]);
    let (tx, rx) = mpsc::channel();
    let server = std::thread::spawn(move || serve(listener, vec![turn1, turn2], tx));

    let dir = std::env::temp_dir().join(format!("ducad-cli-chat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_ducad-cli"))
        .args([
            "chat",
            "--instruction",
            "buat blok 10x20x30",
            "--provider",
            "ollama",
            "--base-url",
            &format!("http://127.0.0.1:{port}/v1"),
            "--model",
            "palsu",
            "--root",
            dir.to_str().unwrap(),
            "--out",
            "blok.ducad",
        ])
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["answer"], "Blok 10×20×30 selesai.");
    assert_eq!(report["tool_calls"][0]["name"], "run_ops");
    assert_eq!(report["tool_calls"][0]["is_error"], false, "{report}");
    assert!(dir.join("blok.ducad").exists());

    let first = rx.recv().unwrap();
    let names: Vec<&str> = first["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["function"]["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"run_ops") && names.contains(&"render_view"),
        "{names:?}"
    );
    assert_eq!(first["max_tokens"], 8000);
    let second = rx.recv().unwrap();
    let msgs = second["messages"].as_array().unwrap();
    let tool_msg = msgs.iter().find(|m| m["role"] == "tool").unwrap();
    assert!(
        tool_msg["content"]
            .as_str()
            .unwrap()
            .contains("\"committed\":true"),
        "{tool_msg}"
    );

    let insp = Command::new(env!("CARGO_BIN_EXE_ducad-cli"))
        .args([
            "inspect",
            dir.join("blok.ducad").to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&insp.stdout).unwrap();
    let vol = v["bodies"][0]["volume"].as_f64().unwrap();
    assert!((vol - 6000.0).abs() < 1e-3, "{vol}");
    let _ = std::fs::remove_dir_all(&dir);
}
