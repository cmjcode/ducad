//! Transport MCP: JSON-RPC 2.0, satu pesan per baris (pola
//! `MNEMONIC/src/api/mcp.rs`). stdout hanya berisi balasan protokol.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Component, Path, PathBuf};

use ducad_engine::{OpError, OpErrorCode, OpResult, Session};
use serde_json::{json, Value};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SERVER_NAME: &str = "ducad";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub const INSTRUCTIONS: &str = "DUCAD adalah CAD B-rep parametrik. Satuan mm, sudut derajat. Alur kerja: (1) `get_schema` sekali untuk melihat format `Op` dan tata bahasa selector; (2) `new_part` atau `open_part`; (3) tulis checks dari persyaratan user dengan `set_checks` sebelum memodelkan; (4) `run_ops` dengan `dry_run: true` untuk memvalidasi, lalu tanpa `dry_run`; (5) `inspect`, `run_checks`, dan `render_view` untuk memverifikasi hasil terhadap spesifikasi; (6) `save_part`. Body dirujuk dengan `id` op pembuatnya. Face/tepi dirujuk dengan selector seperti `>Z`, `|Z`, `of(>Z)`, `all[kind=cylinder][r=2.75]`; uji selector dengan `query_geometry` sebelum dipakai. Batch `run_ops` bersifat atomik: bila satu op gagal, seluruh batch dibatalkan dan `error` menjelaskan penyebab serta `hint`. Ubah dimensi dengan `set_params`, bukan dengan menumpuk op baru.";

/// Satu part terbuka.
pub struct Part {
    pub session: Session,
    /// Path asal (open/save) untuk `save_part` tanpa `path`.
    pub path: Option<PathBuf>,
    pub name: Option<String>,
}

pub struct Server {
    pub sessions: BTreeMap<String, Part>,
    pub next_id: u32,
    /// Semua path tool harus berada di dalam direktori ini.
    pub root: PathBuf,
}

impl Server {
    pub fn new(root: PathBuf) -> anyhow::Result<Self> {
        let root = root.canonicalize()?;
        Ok(Self {
            sessions: BTreeMap::new(),
            next_id: 1,
            root,
        })
    }

    /// Resolusi path tool relatif ke `root`; hasil kanonik HARUS di dalam
    /// `root` (pagar keamanan). Path yang belum ada dikanonisasi lewat
    /// induknya.
    pub fn resolve(&self, p: &str) -> OpResult<PathBuf> {
        let outside = || OpError::new(OpErrorCode::Io, format!("path di luar root: {p}"));
        let raw = Path::new(p);
        let joined = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            self.root.join(raw)
        };
        let canonical = match joined.canonicalize() {
            Ok(c) => c,
            Err(_) => {
                let file = joined.file_name().ok_or_else(outside)?;
                if Path::new(file)
                    .components()
                    .any(|c| matches!(c, Component::ParentDir))
                {
                    return Err(outside());
                }
                let parent = joined.parent().ok_or_else(outside)?;
                let parent = parent.canonicalize().map_err(|e| {
                    OpError::new(
                        OpErrorCode::Io,
                        format!("direktori {} tidak ada: {e}", parent.display()),
                    )
                })?;
                parent.join(file)
            }
        };
        if !canonical.starts_with(&self.root) {
            return Err(outside());
        }
        Ok(canonical)
    }

    /// Pilih sesi: eksplisit, atau satu-satunya sesi yang ada.
    pub fn pick(&mut self, id: Option<&str>) -> OpResult<(String, &mut Part)> {
        let key = match id {
            Some(k) => k.to_string(),
            None => match self.sessions.len() {
                0 => {
                    return Err(
                        OpError::new(OpErrorCode::UnknownRef, "belum ada part terbuka")
                            .with_hint("panggil new_part atau open_part"),
                    )
                }
                1 => self.sessions.keys().next().cloned().unwrap_or_default(),
                _ => {
                    let ids: Vec<&String> = self.sessions.keys().collect();
                    return Err(OpError::new(
                        OpErrorCode::InvalidParam,
                        format!("ada beberapa part terbuka {ids:?}; isi argumen 'session'"),
                    ));
                }
            },
        };
        let known: Vec<String> = self.sessions.keys().cloned().collect();
        match self.sessions.get_mut(&key) {
            Some(p) => Ok((key, p)),
            None => Err(OpError::new(
                OpErrorCode::UnknownRef,
                format!("sesi '{key}' tidak dikenal (yang ada: {known:?})"),
            )),
        }
    }

    pub fn insert(&mut self, part: Part) -> String {
        let id = format!("s{}", self.next_id);
        self.next_id += 1;
        self.sessions.insert(id.clone(), part);
        id
    }
}

/// Baca permintaan baris demi baris sampai EOF; satu baris balasan per
/// permintaan (notifikasi tidak dibalas).
pub fn serve(
    server: &mut Server,
    input: impl BufRead,
    mut output: impl Write,
) -> anyhow::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle_message(server, &line) {
            serde_json::to_writer(&mut output, &reply)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// Tangani satu pesan JSON-RPC mentah. `None` = tidak ada balasan.
pub fn handle_message(server: &mut Server, raw: &str) -> Option<Value> {
    let msg: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            return Some(error_reply(
                Value::Null,
                PARSE_ERROR,
                &format!("parse error: {e}"),
            ))
        }
    };
    let Some(obj) = msg.as_object() else {
        return Some(error_reply(
            Value::Null,
            INVALID_REQUEST,
            "request harus objek",
        ));
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return Some(error_reply(
            id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "method tidak ada",
        ));
    };
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    let result = dispatch(server, method, params);
    let Some(id) = id else {
        if let Err((code, m)) = &result {
            log::warn!("mcp: notifikasi {method} gagal ({code}): {m}");
        }
        return None;
    };
    Some(match result {
        Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
        Err((code, m)) => error_reply(id, code, &m),
    })
}

fn error_reply(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn dispatch(server: &mut Server, method: &str, params: Value) -> Result<Value, (i64, String)> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
            "instructions": INSTRUCTIONS,
        })),
        m if m.starts_with("notifications/") => Ok(Value::Null),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": crate::tools::definitions() })),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or((INVALID_PARAMS, "tools/call butuh 'name'".to_string()))?;
            if !crate::tools::TOOL_NAMES.contains(&name) {
                return Err((INVALID_PARAMS, format!("tool tidak dikenal: {name}")));
            }
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Ok(crate::tools::call(server, name, args))
        }
        _ => Err((
            METHOD_NOT_FOUND,
            format!("method tidak ditemukan: {method}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> (Server, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ducad-mcp-unit-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        (Server::new(dir.clone()).unwrap(), dir)
    }

    fn call(s: &mut Server, id: u64, name: &str, args: Value) -> Value {
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": name, "arguments": args } });
        handle_message(s, &msg.to_string()).unwrap()["result"].clone()
    }

    fn text(result: &Value) -> Value {
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn initialize_and_tools_list() {
        let (mut s, _) = server();
        let r = handle_message(
            &mut s,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        )
        .unwrap();
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(r["result"]["instructions"], INSTRUCTIONS);
        let r =
            handle_message(&mut s, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        let tools = r["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 17);
        for t in tools {
            assert!(t["inputSchema"].is_object(), "{}", t["name"]);
            assert_eq!(t["inputSchema"]["additionalProperties"], false);
            assert!(t["description"].as_str().unwrap().len() > 10);
        }
    }

    #[test]
    fn full_flow() {
        let (mut s, dir) = server();
        let plate: Value = serde_json::from_str(ducad_engine::ops::EXAMPLE_PLATE).unwrap();
        let ops = plate["ops"].as_array().unwrap().clone();

        let r = call(&mut s, 1, "new_part", json!({}));
        assert_eq!(r["isError"], false);
        assert_eq!(text(&r)["session"], "s1");
        let r = call(
            &mut s,
            2,
            "set_params",
            json!({ "params": plate["params"] }),
        );
        assert_eq!(r["isError"], false, "{r}");
        let r = call(&mut s, 3, "run_ops", json!({ "ops": ops[..2] }));
        assert_eq!(r["isError"], false, "{r}");
        let r = call(
            &mut s,
            4,
            "query_geometry",
            json!({ "body": "plate", "edges": "|Z" }),
        );
        assert_eq!(text(&r)["count"], 4);
        let r = call(&mut s, 5, "run_ops", json!({ "ops": ops[2..] }));
        assert_eq!(r["isError"], false, "{r}");

        let r = call(&mut s, 6, "inspect", json!({}));
        let v = text(&r)["bodies"][0]["volume"].as_f64().unwrap();
        let pi = std::f64::consts::PI;
        let expected =
            60.0 * 40.0 * 8.0 - 4.0 * (1.0 - pi / 4.0) * 9.0 * 8.0 - 4.0 * pi * 2.75 * 2.75 * 8.0;
        assert!((v - expected).abs() / expected < 1e-3, "{v}");

        let r = call(&mut s, 7, "render_view", json!({ "view": "iso" }));
        assert_eq!(r["content"][1]["type"], "image");
        assert_eq!(r["content"][1]["mimeType"], "image/png");

        let r = call(
            &mut s,
            8,
            "measure",
            json!({ "a": { "body": "plate", "face": ">Z" }, "b": { "body": "plate", "face": "<Z" } }),
        );
        assert_eq!(text(&r)["plane_gap"], 8.0);

        let r = call(&mut s, 9, "save_part", json!({ "path": "plate.ducad" }));
        assert_eq!(r["isError"], false, "{r}");
        assert!(dir.join("plate.ducad").exists());
        let r = call(&mut s, 10, "open_part", json!({ "path": "plate.ducad" }));
        assert_eq!(text(&r)["session"], "s2");
        let r = call(&mut s, 11, "get_oplog", json!({ "session": "s2" }));
        assert_eq!(text(&r)["ops"].as_array().unwrap().len(), 4);

        // Dua sesi terbuka → argumen session wajib.
        let r = call(&mut s, 12, "inspect", json!({}));
        assert_eq!(r["isError"], true);
        let r = call(&mut s, 13, "undo", json!({ "session": "s1" }));
        assert_eq!(text(&r)["changed"], true);
        let r = call(
            &mut s,
            20,
            "set_checks",
            json!({ "session": "s2", "checks": [
            {"check": "hole_count", "body": "*", "diameter": 5.5, "expect": 4},
            {"check": "min_wall", "body": "*", "min": 2}
        ] }),
        );
        assert_eq!(r["isError"], false, "{r}");
        assert_eq!(text(&r)["pass"], 2);
        let r = call(
            &mut s,
            21,
            "run_checks",
            json!({ "session": "s2", "checks": [{"check": "body_count", "expect": 5}] }),
        );
        assert_eq!(text(&r)["fail"], 1);
        let r = call(
            &mut s,
            22,
            "run_ops",
            json!({ "session": "s2", "ops": [
            {"op": "primitive", "id": "extra", "shape": {"sphere": {"r": 1}}, "at": [200, 0, 0]}
        ] }),
        );
        assert!(
            text(&r)["checks"].is_array(),
            "BatchReport memuat checks: {r}"
        );
        let r = call(&mut s, 14, "get_schema", json!({}));
        let schema = text(&r);
        assert!(schema["selector_cheatsheet"]
            .as_str()
            .unwrap()
            .contains("of(>Z)"));
        assert!(schema["op_schema"]["definitions"]["Op"].is_object());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failing_run_is_error_with_code() {
        let (mut s, dir) = server();
        call(&mut s, 1, "new_part", json!({}));
        let r = call(
            &mut s,
            2,
            "run_ops",
            json!({ "ops": [{"op":"fillet","id":"f","body":"none","edges":"all","radius":1}] }),
        );
        assert_eq!(r["isError"], true);
        let t = r["content"][0]["text"].as_str().unwrap();
        assert!(t.contains("\"code\":\"unknown_ref\""), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn protocol_edge_cases_and_path_fence() {
        let (mut s, dir) = server();
        assert!(handle_message(
            &mut s,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
        )
        .is_none());
        let r = handle_message(&mut s, "{rusak").unwrap();
        assert_eq!(r["error"]["code"], -32700);
        let r = handle_message(&mut s, r#"{"jsonrpc":"2.0","id":3,"method":"tidak/ada"}"#).unwrap();
        assert_eq!(r["error"]["code"], -32601);
        call(&mut s, 4, "new_part", json!({}));
        let r = call(&mut s, 5, "save_part", json!({ "path": "../luar.ducad" }));
        assert_eq!(r["isError"], true);
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("di luar root"));
        let r = call(&mut s, 6, "save_part", json!({ "path": "/etc/luar.ducad" }));
        assert_eq!(r["isError"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oversized_text_is_truncated() {
        let big = json!({ "items": (0..20000).map(|i| json!({ "i": i, "pad": "xxxxxxxxxx" })).collect::<Vec<_>>() });
        let t = crate::tools::compact_text(big);
        assert!(t.len() <= crate::tools::MAX_TEXT_BYTES);
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(v["truncated"], true);
    }
}
