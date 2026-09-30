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

/// Tambahan `instructions` khusus mode `--attach`.
pub const ATTACH_INSTRUCTIONS: &str = " LIVE MODE: you are connected to the DUCAD app that is open right now; every change is visible to the user immediately and each `run_ops`/`set_params`/`replace_op`/`remove_op` is one undo step. `new_part`/`open_part`/`close_part`, `undo`/`redo`, `export`, `diff`, and `list_parts` are not available (ask the user to do it in the app). `propose_ops` shows the user a colored preview and only returns after the user presses Accept/Reject; `accept_proposal` is not available to agents. When the user says \"this\"/\"the selected one\", call `get_selection`; `document_info` gives the document state; after creating geometry, call `set_view` so the result is visible; `screenshot` shows what the user sees.";

pub const INSTRUCTIONS: &str = "DUCAD is a parametric B-rep CAD. Units: mm, angles in degrees. Workflow: (1) call `get_schema` with no arguments once: a summary of every op kind (required/optional fields), the selector cheatsheet, and the list of tested examples; get one kind in detail with `get_schema {\"op\":\"hole\"}` and a full example with `{\"example\":\"flange\"}`; (2) `new_part` or `open_part`; (3) turn the user's requirements into checks with `set_checks` before modeling; (4) `run_ops` with `dry_run: true` to validate, then without `dry_run`; (5) `inspect`, `run_checks`, and `render_view` to verify the result against the spec; (6) `save_part`. Bodies are referenced by the `id` of the op that created them; a boolean consumes bodies `a` and `b` (use the boolean's `id` afterwards). Faces/edges are referenced with selectors such as `>Z`, `|Z`, `of(>Z)`, `all[kind=cylinder][r=2.75]`; test a selector with `query_geometry` before using it. A `run_ops` batch is atomic: if one op fails the whole batch is rolled back and `error` explains the cause, `op_index`, and a `hint`; when `error.fixes` is present, resend the batch with `patched_op` replacing the failed op. Change dimensions with `set_params`; fix an existing op with `replace_op` and drop it with `remove_op` (both replay the oplog) — do not stack corrective ops or undo repeatedly. Full guide + error code table: resource `ducad://guide`. Reply to the user in their own language.";

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
    /// Mode `--attach` (P5.2): tool diteruskan ke aplikasi yang terbuka
    /// dan server ini tidak memiliki sesi sendiri.
    pub attach: Option<crate::attach::AttachClient>,
}

/// Pagar path berbasis satu direktori root.
pub struct RootPaths(pub PathBuf);

impl ducad_engine::tooling::ToolPaths for RootPaths {
    fn resolve(&self, p: &str) -> OpResult<PathBuf> {
        resolve_in_root(&self.0, p)
    }
}

/// Resolusi path tool relatif ke `root`; hasil kanonik HARUS di dalam
/// `root` (pagar keamanan). Path yang belum ada dikanonisasi lewat induknya.
pub fn resolve_in_root(root: &Path, p: &str) -> OpResult<PathBuf> {
    let outside = || OpError::new(OpErrorCode::Io, format!("path outside root: {p}"));
    let raw = Path::new(p);
    let joined = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        root.join(raw)
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
                    format!("directory {} does not exist: {e}", parent.display()),
                )
            })?;
            parent.join(file)
        }
    };
    if !canonical.starts_with(root) {
        return Err(outside());
    }
    Ok(canonical)
}

impl Server {
    pub fn new(root: PathBuf) -> anyhow::Result<Self> {
        let root = root.canonicalize()?;
        Ok(Self {
            sessions: BTreeMap::new(),
            next_id: 1,
            root,
            attach: None,
        })
    }

    /// Server mode attach: seluruh tool diteruskan ke soket jembatan.
    pub fn attached(root: PathBuf, socket: PathBuf) -> anyhow::Result<Self> {
        let mut s = Self::new(root)?;
        s.attach = Some(crate::attach::AttachClient::new(socket));
        Ok(s)
    }

    /// Resolusi path tool relatif ke `root`; hasil kanonik HARUS di dalam
    /// `root` (pagar keamanan). Path yang belum ada dikanonisasi lewat
    /// induknya.
    pub fn resolve(&self, p: &str) -> OpResult<PathBuf> {
        resolve_in_root(&self.root, p)
    }

    /// Pagar path yang bisa dipegang tanpa meminjam `Server` (dipakai saat
    /// sesi sudah dipinjam secara mutable).
    pub fn paths(&self) -> RootPaths {
        RootPaths(self.root.clone())
    }

    /// Pilih sesi: eksplisit, atau satu-satunya sesi yang ada.
    pub fn pick(&mut self, id: Option<&str>) -> OpResult<(String, &mut Part)> {
        let key = match id {
            Some(k) => k.to_string(),
            None => match self.sessions.len() {
                0 => {
                    return Err(OpError::new(OpErrorCode::UnknownRef, "no part is open")
                        .with_hint("call new_part or open_part"))
                }
                1 => self.sessions.keys().next().cloned().unwrap_or_default(),
                _ => {
                    let ids: Vec<&String> = self.sessions.keys().collect();
                    return Err(OpError::new(
                        OpErrorCode::InvalidParam,
                        format!("several parts are open {ids:?}; pass the 'session' argument"),
                    ));
                }
            },
        };
        let known: Vec<String> = self.sessions.keys().cloned().collect();
        match self.sessions.get_mut(&key) {
            Some(p) => Ok((key, p)),
            None => Err(OpError::new(
                OpErrorCode::UnknownRef,
                format!("unknown session '{key}' (open: {known:?})"),
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
            "request must be an object",
        ));
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return Some(error_reply(
            id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "missing method",
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

/// Panduan lengkap (resource `ducad://guide`), dibangkitkan dari engine
/// supaya selalu sama dengan skema dan kode error yang berlaku.
fn guide_text() -> String {
    let examples: String = ducad_engine::ops::EXAMPLES
        .iter()
        .map(|(n, _, covers)| format!("- `ducad://example/{n}` — {covers}\n"))
        .collect();
    format!(
        "# Modeling with DUCAD\n\n{INSTRUCTIONS}\n\n## Op kinds\n\n{}\n\
         Full fields of one kind: `get_schema {{\"op\":\"<kind>\"}}`.\n\n\
         ## Selectors\n\n```text\n{}```\n\n## Error codes\n\n{}\n## Anti-patterns\n\n{}\n\
         ## Tested examples\n\n{examples}",
        ducad_engine::tooling::op_catalog_markdown(),
        ducad_engine::select::SELECTOR_CHEATSHEET,
        ducad_engine::tooling::ERROR_GUIDE,
        ducad_engine::tooling::ANTI_PATTERNS,
    )
}

fn resource_list(server: &Server) -> Vec<Value> {
    let mut out = vec![
        json!({ "uri": "ducad://schema", "name": "Op schema (JSON Schema)", "mimeType": "application/json" }),
        json!({ "uri": "ducad://guide", "name": "DUCAD modeling guide", "mimeType": "text/markdown" }),
    ];
    for (name, _, covers) in ducad_engine::ops::EXAMPLES {
        out.push(json!({ "uri": format!("ducad://example/{name}"), "name": format!("Example OpFile {name}"),
                         "description": covers, "mimeType": "application/json" }));
    }
    for id in server.sessions.keys() {
        out.push(json!({ "uri": format!("ducad://part/{id}/oplog"), "name": format!("Oplog {id}"), "mimeType": "application/json" }));
        out.push(json!({ "uri": format!("ducad://part/{id}/summary"), "name": format!("Summary {id}"), "mimeType": "application/json" }));
    }
    out
}

fn read_resource(server: &Server, uri: &str) -> Result<Value, String> {
    let text_of = |mime: &str, text: String| json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": text }] });
    match uri {
        "ducad://schema" => Ok(text_of(
            "application/json",
            ducad_engine::ops::op_schema().to_string(),
        )),
        "ducad://guide" => Ok(text_of("text/markdown", guide_text())),
        _ if uri.starts_with("ducad://example/") => {
            let name = uri.trim_start_matches("ducad://example/");
            ducad_engine::ops::example(name)
                .map(|t| text_of("application/json", t.to_string()))
                .ok_or_else(|| format!("unknown example: {name}"))
        }
        _ => {
            let rest = uri
                .strip_prefix("ducad://part/")
                .ok_or_else(|| format!("unknown resource: {uri}"))?;
            let (id, what) = rest
                .split_once('/')
                .ok_or_else(|| format!("unknown resource: {uri}"))?;
            let part = server
                .sessions
                .get(id)
                .ok_or_else(|| format!("unknown session '{id}'"))?;
            let v = match what {
                "oplog" => {
                    let d = part.session.design();
                    json!({ "params": d.params, "ops": d.oplog })
                }
                "summary" => {
                    serde_json::to_value(part.session.summary()).map_err(|e| e.to_string())?
                }
                _ => return Err(format!("unknown resource: {uri}")),
            };
            Ok(text_of("application/json", v.to_string()))
        }
    }
}

fn prompt_list() -> Vec<Value> {
    vec![
        json!({
            "name": "model_part",
            "description": "Create a new part from a spec with the DUCAD workflow (params, checks, dry run, verification).",
            "arguments": [
                { "name": "spec", "description": "Part spec (dimensions, features, material).", "required": true },
                { "name": "path", "description": "Target .ducad file.", "required": false }
            ]
        }),
        json!({
            "name": "fix_error",
            "description": "Recover from a DUCAD tool error (fixes, replace_op, query_geometry) without looping.",
            "arguments": [
                { "name": "error", "description": "The error JSON object from the tool result.", "required": true }
            ]
        }),
        json!({
            "name": "edit_part",
            "description": "Change an existing part without rewriting the oplog (prefer set_params / replace_op).",
            "arguments": [
                { "name": "path", "description": ".ducad file.", "required": true },
                { "name": "change", "description": "The requested change.", "required": true }
            ]
        }),
    ]
}

fn get_prompt(name: &str, args: &Value) -> Result<Value, String> {
    let arg = |k: &str| {
        args.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let (description, text) = match name {
        "model_part" => {
            let spec = arg("spec");
            if spec.trim().is_empty() {
                return Err("argument 'spec' is required".into());
            }
            let path = match arg("path") {
                p if p.is_empty() => "part.ducad".to_string(),
                p => p,
            };
            (
                "Create a new part",
                format!(
                    "Create a DUCAD part from the following spec, then save it to {path}.\n\nSPEC:\n{spec}\n\n\
                     Steps: get_schema → new_part → set_params (every dimension) → set_checks from the requirements → \
                     run_ops dry_run then commit in small batches → inspect + run_checks + render_view (iso and top) → \
                     compare the numbers with the spec → save_part. Report the main dimensions and your assumptions."
                ),
            )
        }
        "edit_part" => {
            let (path, change) = (arg("path"), arg("change"));
            if path.is_empty() || change.trim().is_empty() {
                return Err("arguments 'path' and 'change' are required".into());
            }
            (
                "Change a part",
                format!(
                    "Open {path} with open_part, read get_oplog, then make this change: {change}\n\n\
                     Prefer set_params when a matching param exists; change old ops with replace_op \
                     and drop them with remove_op (dry_run first) — do not stack corrective ops. \
                     Verify with run_checks and render_view, then save_part to the same path."
                ),
            )
        }
        "fix_error" => {
            let error = arg("error");
            if error.trim().is_empty() {
                return Err("argument 'error' is required".into());
            }
            (
                "Recover from an error",
                format!(
                    "A DUCAD tool failed with this error:\n\n{error}\n\n\
                     Steps: (1) read code, message, op_index/op_id, hint, and context; \
                     (2) if fixes is present, resend the batch with fixes[0].patched_op replacing the op at op_index \
                     (mind patch.op_id — a fix may patch another op, e.g. a sketch); \
                     (3) selector_empty/selector_syntax → test a new selector with query_geometry first; \
                     (4) unknown_ref/body_consumed → check names with inspect/get_oplog; \
                     (5) a mistake in an op that is ALREADY committed → replace_op with dry_run:true, then without dry_run. \
                     Do not repeat the same op unchanged more than once."
                ),
            )
        }
        other => return Err(format!("unknown prompt: {other}")),
    };
    Ok(json!({
        "description": description,
        "messages": [{ "role": "user", "content": { "type": "text", "text": text } }]
    }))
}

fn error_reply(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn dispatch(server: &mut Server, method: &str, params: Value) -> Result<Value, (i64, String)> {
    match method {
        "initialize" => {
            let instructions = match server.attach {
                Some(_) => format!("{INSTRUCTIONS}{ATTACH_INSTRUCTIONS}"),
                None => INSTRUCTIONS.to_string(),
            };
            Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
                "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
                "instructions": instructions,
            }))
        }
        m if m.starts_with("notifications/") => Ok(Value::Null),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": match server.attach {
            Some(_) => crate::tools::chat_tools(true),
            None => crate::tools::definitions(),
        } })),
        "resources/list" => Ok(json!({ "resources": resource_list(server) })),
        "resources/read" => {
            let uri = params
                .get("uri")
                .and_then(Value::as_str)
                .ok_or((INVALID_PARAMS, "resources/read requires 'uri'".to_string()))?;
            read_resource(server, uri).map_err(|m| (INVALID_PARAMS, m))
        }
        "prompts/list" => Ok(json!({ "prompts": prompt_list() })),
        "prompts/get" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or((INVALID_PARAMS, "prompts/get requires 'name'".to_string()))?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            get_prompt(name, &args).map_err(|m| (INVALID_PARAMS, m))
        }
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or((INVALID_PARAMS, "tools/call requires 'name'".to_string()))?;
            let live_ok = server.attach.is_some() && crate::tools::LIVE_TOOL_NAMES.contains(&name);
            if !crate::tools::TOOL_NAMES.contains(&name) && !live_ok {
                return Err((INVALID_PARAMS, format!("unknown tool: {name}")));
            }
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Ok(crate::tools::call(server, name, args))
        }
        _ => Err((METHOD_NOT_FOUND, format!("method not found: {method}"))),
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
        assert_eq!(tools.len(), crate::tools::TOOL_NAMES.len());
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
        let r = call(
            &mut s,
            30,
            "propose_ops",
            json!({ "session": "s2", "ops": [
            {"op": "shell", "id": "sh", "body": "plate", "remove_faces": "<Z", "thickness": 1}
        ] }),
        );
        assert_eq!(r["isError"], false, "{r}");
        assert_eq!(r["content"][1]["type"], "image");
        let pid = text(&r)["proposal_id"].as_str().unwrap().to_string();
        let r = call(
            &mut s,
            31,
            "accept_proposal",
            json!({ "session": "s2", "proposal_id": pid }),
        );
        assert_eq!(r["isError"], false, "{r}");
        assert_eq!(text(&r)["committed"], true);
        let r = call(
            &mut s,
            32,
            "reject_proposal",
            json!({ "session": "s2", "proposal_id": "p99" }),
        );
        assert_eq!(text(&r)["rejected"], false);
        let r = call(&mut s, 14, "get_schema", json!({}));
        let schema = text(&r);
        assert!(schema["selector_cheatsheet"]
            .as_str()
            .unwrap()
            .contains("of(>Z)"));
        assert!(schema["ops"].as_array().is_some_and(|a| a.len() >= 19));
        let r = call(&mut s, 15, "get_schema", json!({ "full": true }));
        assert!(text(&r)["op_schema"]["definitions"]["Op"].is_object());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tools_list_is_compact_and_annotated() {
        let (mut s, _) = server();
        let r =
            handle_message(&mut s, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
        let size = r.to_string().len();
        assert!(size < 40 * 1024, "tools/list terlalu besar: {size} byte");
        for t in r["result"]["tools"].as_array().unwrap() {
            let name = t["name"].as_str().unwrap();
            assert!(t["title"].is_string(), "{name}");
            let a = &t["annotations"];
            assert!(a["readOnlyHint"].is_boolean(), "{name}");
            assert_eq!(a["openWorldHint"], false, "{name}");
        }
        let find = |n: &str| {
            r["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["name"] == n)
                .cloned()
                .unwrap()
        };
        assert_eq!(find("inspect")["annotations"]["readOnlyHint"], true);
        assert_eq!(find("remove_op")["annotations"]["destructiveHint"], true);
        let kinds = &find("run_ops")["inputSchema"]["properties"]["ops"]["items"]["properties"]
            ["op"]["enum"];
        assert!(kinds.as_array().unwrap().contains(&json!("helix")));
    }

    #[test]
    fn edit_tools_change_existing_ops() {
        let (mut s, dir) = server();
        let plate: Value = serde_json::from_str(ducad_engine::ops::EXAMPLE_PLATE).unwrap();
        call(&mut s, 1, "new_part", json!({}));
        call(
            &mut s,
            2,
            "set_params",
            json!({ "params": plate["params"] }),
        );
        let r = call(&mut s, 3, "run_ops", json!({ "ops": plate["ops"] }));
        assert_eq!(r["isError"], false, "{r}");
        let vol = |r: &Value| text(r)["summary"]["bodies"][0]["volume"].as_f64().unwrap();
        let v0 = vol(&r);

        // Salah ketik field → op_index menunjuk op yang salah.
        let r = call(
            &mut s,
            4,
            "run_ops",
            json!({ "ops": [
            {"op":"primitive","id":"x","shape":{"sphere":{"r":1}}},
            {"op":"chamfer","id":"c","body":"plate","edges":"|Z","distanse":1}
        ] }),
        );
        assert_eq!(r["isError"], true);
        assert_eq!(text(&r)["error"]["op_index"], 1, "{r}");

        let fillet = json!({"op":"fillet","id":"f1","body":"plate","edges":"|Z","radius":1});
        let r = call(
            &mut s,
            5,
            "replace_op",
            json!({ "id": "f1", "op": fillet, "dry_run": true }),
        );
        assert_eq!(text(&r)["committed"], false, "{r}");
        assert!(vol(&r) > v0);
        let r = call(&mut s, 6, "replace_op", json!({ "id": "f1", "op": fillet }));
        assert_eq!(text(&r)["committed"], true, "{r}");
        let v1 = vol(&r);
        assert!(v1 > v0);

        let r = call(&mut s, 7, "remove_op", json!({ "ids": ["base"] }));
        assert_eq!(r["isError"], true);
        assert!(
            text(&r)["error"]["hint"]
                .as_str()
                .unwrap()
                .contains("replace_op"),
            "{r}"
        );
        let r = call(&mut s, 8, "remove_op", json!({ "ids": ["h1"] }));
        assert_eq!(text(&r)["committed"], true, "{r}");
        let r = call(&mut s, 9, "get_oplog", json!({}));
        assert_eq!(text(&r)["ops"].as_array().unwrap().len(), 3);

        // Proposal edit: params + ganti op, diterapkan lewat accept.
        let r = call(
            &mut s,
            10,
            "propose_ops",
            json!({
                "params": { "t": 10 },
                "replace": [{ "id": "f1", "op": {"op":"fillet","id":"f1","body":"plate","edges":"|Z","radius":2} }]
            }),
        );
        assert_eq!(r["isError"], false, "{r}");
        assert_eq!(r["content"][1]["type"], "image");
        let pid = text(&r)["proposal_id"].as_str().unwrap().to_string();
        let r = call(&mut s, 11, "accept_proposal", json!({ "proposal_id": pid }));
        assert_eq!(text(&r)["committed"], true, "{r}");
        let r = call(&mut s, 12, "get_oplog", json!({}));
        assert_eq!(text(&r)["params"]["t"], 10.0);
        assert_eq!(text(&r)["params"]["w"], 60.0, "params lama tetap");

        let r = call(&mut s, 13, "propose_ops", json!({}));
        assert_eq!(r["isError"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn guide_and_examples_resources() {
        let (mut s, _) = server();
        let r = handle_message(
            &mut s,
            r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"ducad://guide"}}"#,
        )
        .unwrap();
        let guide = r["result"]["contents"][0]["text"].as_str().unwrap();
        for needle in [
            "**helix**",
            "fillet_radius_too_large",
            "replace_op",
            "ducad://example/flange",
        ] {
            assert!(guide.contains(needle), "panduan tanpa {needle}");
        }
        for (name, _, _) in ducad_engine::ops::EXAMPLES {
            let msg = json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/read",
                              "params": { "uri": format!("ducad://example/{name}") } });
            let r = handle_message(&mut s, &msg.to_string()).unwrap();
            assert!(
                r["result"]["contents"][0]["text"].is_string(),
                "{name}: {r}"
            );
        }
        let r = handle_message(
            &mut s,
            r#"{"jsonrpc":"2.0","id":3,"method":"prompts/get","params":{"name":"fix_error","arguments":{"error":"{\"code\":\"selector_empty\"}"}}}"#,
        )
        .unwrap();
        assert!(r["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("query_geometry"));
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
            .contains("outside root"));
        let r = call(&mut s, 6, "save_part", json!({ "path": "/etc/luar.ducad" }));
        assert_eq!(r["isError"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resources_prompts_and_new_tools() {
        let (mut s, dir) = server();
        let r = handle_message(
            &mut s,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        )
        .unwrap();
        assert!(r["result"]["capabilities"]["resources"].is_object());
        call(&mut s, 2, "new_part", json!({}));
        let r = handle_message(
            &mut s,
            r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#,
        )
        .unwrap();
        let uris: Vec<String> = r["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["uri"].as_str().unwrap().to_string())
            .collect();
        assert!(
            uris.contains(&"ducad://part/s1/oplog".to_string()),
            "{uris:?}"
        );
        let r = handle_message(
            &mut s,
            r#"{"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"ducad://guide"}}"#,
        )
        .unwrap();
        assert!(r["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("of(>Z)"));
        let r = handle_message(
            &mut s,
            r#"{"jsonrpc":"2.0","id":5,"method":"resources/read","params":{"uri":"ducad://part/zz/oplog"}}"#,
        )
        .unwrap();
        assert_eq!(r["error"]["code"], -32602);
        let r = handle_message(
            &mut s,
            r#"{"jsonrpc":"2.0","id":6,"method":"prompts/get","params":{"name":"model_part","arguments":{"spec":"plat 60x40x8"}}}"#,
        )
        .unwrap();
        assert!(r["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("plat 60x40x8"));

        // Part: blok → gambar kerja PDF → ekspor STEP → impor ulang → diff.
        let r = call(
            &mut s,
            7,
            "run_ops",
            json!({ "ops": [
            {"op":"primitive","id":"blok","shape":{"box":{"size":[30,20,10]}}}
        ] }),
        );
        assert_eq!(r["isError"], false, "{r}");
        let r = call(
            &mut s,
            8,
            "drawing",
            json!({ "format": "pdf", "path": "blok.pdf", "title": "Blok" }),
        );
        assert_eq!(r["isError"], false, "{r}");
        assert!(dir.join("blok.pdf").metadata().unwrap().len() > 500);
        let r = call(
            &mut s,
            9,
            "export",
            json!({ "format": "step", "path": "blok.step" }),
        );
        assert_eq!(r["isError"], false, "{r}");
        let r = call(&mut s, 10, "save_part", json!({ "path": "v1.ducad" }));
        assert_eq!(r["isError"], false, "{r}");
        let r = call(
            &mut s,
            11,
            "import_step",
            json!({ "path": "blok.step", "name": "salinan" }),
        );
        assert_eq!(r["isError"], false, "{r}");
        assert!((text(&r)["volume"].as_f64().unwrap() - 6000.0).abs() < 1e-3);
        let r = call(
            &mut s,
            12,
            "import_step",
            json!({ "path": "blok.step", "name": "salinan" }),
        );
        assert_eq!(r["isError"], true, "nama ganda ditolak");
        let r = call(&mut s, 13, "diff", json!({ "against_path": "v1.ducad" }));
        assert_eq!(r["isError"], false, "{r}");
        let r = call(&mut s, 14, "list_parts", json!({}));
        assert_eq!(text(&r)["parts"][0]["bodies"], 2);
        // Impor tersimpan di part dan ikut di-replay.
        call(&mut s, 15, "save_part", json!({ "path": "v2.ducad" }));
        let r = call(&mut s, 16, "open_part", json!({ "path": "v2.ducad" }));
        assert_eq!(
            text(&r)["summary"]["bodies"].as_array().unwrap().len(),
            2,
            "{r}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oversized_text_is_truncated() {
        let big = json!({ "items": (0..20000).map(|i| json!({ "i": i, "pad": "xxxxxxxxxx" })).collect::<Vec<_>>() });
        let t = ducad_engine::tooling::compact_text(big);
        assert!(t.len() <= ducad_engine::tooling::MAX_TEXT_BYTES);
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(v["truncated"], true);
    }
}
