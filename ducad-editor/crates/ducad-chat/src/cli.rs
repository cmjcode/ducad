//! CLI coding agent sebagai backend chat (P13.5), pola harness TABULAR
//! (`tabular-client/src/agent/harness.rs`).
//!
//! Agent (Antigravity `agy`, Claude Code `claude`, Gemini CLI `gemini`, atau
//! perintah kustom) dijalankan dalam mode print + `stream-json` di direktori
//! kerja kosong `~/.ducad/agent-workspace`. Ia mengendalikan DUCAD lewat
//! server MCP `ducad-mcp --attach` yang tersambung ke jembatan aplikasi, jadi
//! tool, pagar, dan satu-batch-satu-undo sama dengan chat API.
//!
//! Desktop saja: iPadOS tidak bisa menjalankan proses anak.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::ChatEvent;

/// Nama server MCP DUCAD di konfigurasi CLI (`mcp__ducad__run_ops`).
pub const MCP_SERVER_NAME: &str = "ducad";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliAgentKind {
    #[default]
    Antigravity,
    ClaudeCode,
    GeminiCli,
    Custom,
}

impl CliAgentKind {
    pub const ALL: [CliAgentKind; 4] = [
        CliAgentKind::Antigravity,
        CliAgentKind::ClaudeCode,
        CliAgentKind::GeminiCli,
        CliAgentKind::Custom,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            CliAgentKind::Antigravity => "Antigravity (agy)",
            CliAgentKind::ClaudeCode => "Claude Code (claude)",
            CliAgentKind::GeminiCli => "Gemini CLI (gemini)",
            CliAgentKind::Custom => "Custom command",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            CliAgentKind::Antigravity => "agy",
            CliAgentKind::ClaudeCode => "claude",
            CliAgentKind::GeminiCli => "gemini",
            CliAgentKind::Custom => "custom",
        }
    }

    /// Nama binary yang dicari bila path kosong.
    pub fn default_binary(self) -> &'static str {
        match self {
            CliAgentKind::Antigravity => "agy",
            CliAgentKind::ClaudeCode => "claude",
            CliAgentKind::GeminiCli => "gemini",
            CliAgentKind::Custom => "",
        }
    }

    /// Pilihan cepat model (kosong = bawaan akun CLI).
    pub fn preset_models(self) -> &'static [&'static str] {
        match self {
            CliAgentKind::Antigravity => &[
                "gemini-3.8-flash-medium",
                "gemini-3.8-flash-high",
                "gemini-3.1-pro-high",
                "claude-sonnet-4-6",
                "claude-opus-4-6-thinking",
            ],
            CliAgentKind::ClaudeCode => &["sonnet", "opus", "haiku"],
            CliAgentKind::GeminiCli => &["gemini-2.5-pro", "gemini-2.5-flash"],
            CliAgentKind::Custom => &[],
        }
    }

    /// Tingkat effort yang diterima (`--effort`); kosong = tidak didukung.
    pub fn effort_levels(self) -> &'static [&'static str] {
        match self {
            CliAgentKind::Antigravity => &["low", "medium", "high", "max"],
            CliAgentKind::ClaudeCode => &["low", "medium", "high", "xhigh", "max"],
            _ => &[],
        }
    }

    /// CLI menyimpan percakapan dan bisa dilanjutkan dengan id sesi.
    pub fn supports_resume(self) -> bool {
        matches!(self, CliAgentKind::Antigravity | CliAgentKind::ClaudeCode)
    }

    /// Server MCP dipasang lewat registrasi global (`<cli> mcp add`), bukan
    /// berkas konfigurasi per giliran.
    pub fn needs_global_mcp(self) -> bool {
        matches!(self, CliAgentKind::Antigravity | CliAgentKind::GeminiCli)
    }
}

/// Profil satu CLI agent (disimpan di `ai-chat.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CliAgentProfile {
    pub kind: CliAgentKind,
    #[serde(default)]
    pub enabled: bool,
    /// Path binary; kosong = cari `kind.default_binary()`.
    #[serde(default)]
    pub bin: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub effort: String,
    /// Argumen tambahan. Untuk `Custom`: templat dengan `{prompt}`,
    /// `{system}`, `{model}`, `{session}`, `{mcp_config}`.
    #[serde(default)]
    pub extra_args: String,
}

impl CliAgentKind {
    /// Nama data di `ai-chat.json` (`config::CLI_KINDS`).
    pub fn data_name(self) -> &'static str {
        match self {
            CliAgentKind::Antigravity => "antigravity",
            CliAgentKind::ClaudeCode => "claude_code",
            CliAgentKind::GeminiCli => "gemini_cli",
            CliAgentKind::Custom => "custom",
        }
    }

    pub fn from_data_name(s: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|k| k.data_name() == s)
            .unwrap_or_default()
    }
}

impl From<&crate::config::CliProfileData> for CliAgentProfile {
    fn from(d: &crate::config::CliProfileData) -> Self {
        Self {
            kind: CliAgentKind::from_data_name(&d.kind),
            enabled: d.enabled,
            bin: d.bin.clone(),
            model: d.model.clone(),
            effort: d.effort.clone(),
            extra_args: d.extra_args.clone(),
        }
    }
}

impl CliAgentProfile {
    pub fn new(kind: CliAgentKind) -> Self {
        Self {
            kind,
            ..Default::default()
        }
    }

    pub fn effective_bin(&self) -> String {
        match self.bin.trim() {
            "" => self.kind.default_binary().to_string(),
            b => b.to_string(),
        }
    }
}

/// Satu giliran ke CLI.
#[derive(Debug, Clone)]
pub struct CliRequest {
    pub system_prompt: String,
    pub user_prompt: String,
    pub session_id: Option<String>,
    pub cwd: PathBuf,
    /// Berkas konfigurasi MCP (`--mcp-config` Claude Code, `{mcp_config}` kustom).
    pub mcp_config: Option<PathBuf>,
}

/// Pisahkan argumen ala shell (spasi; kutip tunggal/ganda; backslash).
pub fn split_args(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut has = false;
    for ch in input.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
            has = true;
            continue;
        }
        match (quote, ch) {
            (_, '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"') | (None, '\'') => {
                quote = Some(ch);
                has = true;
            }
            (None, c) if c.is_whitespace() => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (None, c) => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

/// Nama model agy yang sudah membawa effort (`…-low|-medium|-high`).
pub fn model_embeds_effort(model: &str) -> bool {
    matches!(
        model.trim().rsplit('-').next(),
        Some("low" | "medium" | "high")
    )
}

fn combined_prompt(req: &CliRequest) -> String {
    if req.system_prompt.trim().is_empty() {
        req.user_prompt.clone()
    } else {
        format!(
            "{}\n\n---\n\n{}",
            req.system_prompt.trim_end(),
            req.user_prompt
        )
    }
}

/// Susun argumen baris perintah (terpisah dari spawn supaya bisa dites).
pub fn build_args(p: &CliAgentProfile, req: &CliRequest) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let model = p.model.trim();
    let effort = p.effort.trim();
    let session = req
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match p.kind {
        CliAgentKind::Antigravity => {
            a.extend(["--print".into(), combined_prompt(req)]);
            a.extend(["--output-format".into(), "stream-json".into()]);
            // Mode print tidak bisa menjawab prompt izin; cwd = folder kosong
            // khusus dan satu-satunya server MCP yang relevan adalah DUCAD.
            a.push("--dangerously-skip-permissions".into());
            if !model.is_empty() {
                a.extend(["--model".into(), model.into()]);
            }
            if !effort.is_empty() && !model_embeds_effort(model) {
                a.extend(["--effort".into(), effort.into()]);
            }
            if let Some(id) = session {
                a.extend(["--conversation".into(), id.into()]);
            }
        }
        CliAgentKind::ClaudeCode => {
            a.extend(["-p".into(), req.user_prompt.clone()]);
            a.extend(["--output-format".into(), "stream-json".into()]);
            a.push("--verbose".into());
            a.push("--include-partial-messages".into());
            if !req.system_prompt.trim().is_empty() {
                a.extend(["--append-system-prompt".into(), req.system_prompt.clone()]);
            }
            if !model.is_empty() {
                a.extend(["--model".into(), model.into()]);
            }
            if !effort.is_empty() {
                a.extend(["--effort".into(), effort.into()]);
            }
            if let Some(id) = session {
                a.extend(["--resume".into(), id.into()]);
            }
            if let Some(path) = &req.mcp_config {
                a.extend(["--mcp-config".into(), path.to_string_lossy().to_string()]);
                a.push("--strict-mcp-config".into());
                // Hanya tool MCP DUCAD yang boleh tanpa prompt; tool lain
                // (Bash, Edit, …) otomatis ditolak di mode print.
                a.extend(["--allowedTools".into(), format!("mcp__{MCP_SERVER_NAME}")]);
            }
        }
        CliAgentKind::GeminiCli => {
            a.extend(["-p".into(), combined_prompt(req)]);
            a.extend(["--output-format".into(), "stream-json".into()]);
            // Tool MCP DUCAD mengubah model, jadi mode read-only (`plan`) tidak
            // cukup; cwd adalah folder kosong dan hanya server `ducad` yang
            // diizinkan. Bisa ditimpa lewat argumen tambahan (nilai terakhir menang).
            a.extend(["--approval-mode".into(), "yolo".into()]);
            a.extend(["--allowed-mcp-server-names".into(), MCP_SERVER_NAME.into()]);
            if !model.is_empty() {
                a.extend(["-m".into(), model.into()]);
            }
        }
        CliAgentKind::Custom => {
            let mcp = req
                .mcp_config
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut used = false;
            for tok in split_args(&p.extra_args) {
                let t = tok
                    .replace("{system}", &req.system_prompt)
                    .replace("{model}", model)
                    .replace("{session}", session.unwrap_or(""))
                    .replace("{mcp_config}", &mcp);
                if t.contains("{prompt}") {
                    used = true;
                    a.push(t.replace("{prompt}", &req.user_prompt));
                } else {
                    a.push(t);
                }
            }
            if !used {
                a.push(combined_prompt(req));
            }
            return a;
        }
    }
    a.extend(split_args(&p.extra_args));
    a
}

/// Kejadian satu giliran CLI.
#[derive(Debug, Clone, PartialEq)]
pub enum CliEvent {
    /// Id sesi/percakapan untuk melanjutkan giliran berikutnya.
    Session(String),
    /// Teks, tool mulai/selesai, atau error (bentuk sama dengan chat API).
    Chat(ChatEvent),
    /// Giliran selesai.
    Finished { text: String, usage: Option<String> },
}

/// Nama tool tanpa awalan MCP (`mcp__ducad__run_ops` → `run_ops`).
pub fn short_tool_name(name: &str) -> String {
    for prefix in [
        format!("mcp__{MCP_SERVER_NAME}__"),
        format!("{MCP_SERVER_NAME}__"),
        format!("{MCP_SERVER_NAME}."),
        format!("{MCP_SERVER_NAME}/"),
    ] {
        if let Some(rest) = name.strip_prefix(&prefix) {
            return rest.to_string();
        }
    }
    name.to_string()
}

fn ellipsize(s: &str, max: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= max {
        one
    } else {
        format!("{}…", one.chars().take(max).collect::<String>())
    }
}

fn format_usage(usage: &Value, secs: Option<f64>) -> Option<String> {
    let mut parts = Vec::new();
    match (
        usage["input_tokens"].as_u64(),
        usage["output_tokens"].as_u64(),
    ) {
        (Some(i), Some(o)) => parts.push(format!("{i} masuk / {o} keluar token")),
        _ => {
            if let Some(t) = usage["total_tokens"].as_u64() {
                parts.push(format!("{t} token"));
            }
        }
    }
    if let Some(s) = secs {
        parts.push(format!("{s:.1} s"));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// Pengurai NDJSON stateful, satu per giliran. Event tak dikenal diabaikan
/// supaya perubahan kecil antar versi CLI tidak mematahkan chat.
pub struct StreamParser {
    kind: CliAgentKind,
    text: String,
    saw_delta: bool,
    saw_stream_event: bool,
    saw_json: bool,
    finished: bool,
    raw_lines: String,
    /// id tool → nama, untuk menutup kartu saat hasil tiba.
    open_tools: HashMap<String, String>,
}

impl StreamParser {
    pub fn new(kind: CliAgentKind) -> Self {
        Self {
            kind,
            text: String::new(),
            saw_delta: false,
            saw_stream_event: false,
            saw_json: false,
            finished: false,
            raw_lines: String::new(),
            open_tools: HashMap::new(),
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    fn delta(&mut self, s: &str) -> CliEvent {
        self.saw_delta = true;
        self.text.push_str(s);
        CliEvent::Chat(ChatEvent::TextDelta {
            text: s.to_string(),
        })
    }

    fn start_tool(&mut self, id: String, name: &str, input: &Value) -> CliEvent {
        let short = short_tool_name(name);
        self.open_tools.insert(id.clone(), short.clone());
        CliEvent::Chat(ChatEvent::ToolStart {
            id,
            name: short,
            input: input.clone(),
        })
    }

    fn finish_tool(
        &mut self,
        id: &str,
        error: Option<String>,
        summary: String,
    ) -> Option<CliEvent> {
        let name = self.open_tools.remove(id)?;
        let is_error = error.is_some();
        Some(CliEvent::Chat(ChatEvent::ToolDone {
            id: id.to_string(),
            name,
            summary: error.unwrap_or(summary),
            is_error,
            image_png: None,
        }))
    }

    fn done(&mut self, fallback: Option<&str>, usage: Option<String>) -> CliEvent {
        self.finished = true;
        let text = if self.saw_delta {
            self.text.clone()
        } else {
            fallback.unwrap_or("").to_string()
        };
        CliEvent::Finished { text, usage }
    }

    fn error(&mut self, msg: String) -> CliEvent {
        self.finished = true;
        // Pesan API mentah (JSON bertingkat) → instruksi login bila cocok.
        let message = detect_login_problem(self.kind, &msg).unwrap_or(msg);
        CliEvent::Chat(ChatEvent::Error { message })
    }

    /// Satu baris stdout → 0..n kejadian.
    pub fn feed_line(&mut self, line: &str) -> Vec<CliEvent> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() || self.finished {
            return Vec::new();
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            if self.kind == CliAgentKind::Custom {
                return vec![self.delta(&format!("{line}\n"))];
            }
            self.raw_lines.push_str(line);
            self.raw_lines.push('\n');
            return Vec::new();
        };
        self.saw_json = true;
        match self.kind {
            CliAgentKind::Antigravity => self.feed_agy(&v),
            CliAgentKind::ClaudeCode => self.feed_claude(&v),
            CliAgentKind::GeminiCli => self.feed_gemini(&v),
            CliAgentKind::Custom => {
                if v.get("event").is_some() {
                    self.feed_agy(&v)
                } else if matches!(
                    v["type"].as_str(),
                    Some("stream_event" | "assistant" | "system" | "user")
                ) || (v["type"] == "result" && v.get("session_id").is_some())
                {
                    self.feed_claude(&v)
                } else if v.get("type").is_some() {
                    self.feed_gemini(&v)
                } else {
                    vec![self.delta(&format!("{line}\n"))]
                }
            }
        }
    }

    /// stdout ditutup. `Some` bila giliran belum ditutup event `result`.
    pub fn finish(&mut self) -> Option<CliEvent> {
        if self.finished {
            return None;
        }
        if self.saw_delta {
            return Some(self.done(None, None));
        }
        if !self.saw_json && !self.raw_lines.trim().is_empty() {
            let raw = self.raw_lines.trim().to_string();
            return Some(self.done(Some(&raw), None));
        }
        None
    }

    fn feed_agy(&mut self, v: &Value) -> Vec<CliEvent> {
        let mut out = Vec::new();
        match v["event"].as_str().unwrap_or("") {
            "init" => {
                if let Some(id) = v["conversation_id"].as_str() {
                    out.push(CliEvent::Session(id.to_string()));
                }
            }
            "step_update" => {
                let su = &v["step_update"];
                match su["step_type"].as_str().unwrap_or("") {
                    "agent_response" => {
                        if let Some(d) = su["text_delta"].as_str().filter(|d| !d.is_empty()) {
                            out.push(self.delta(d));
                        }
                    }
                    "user_input" | "" => {}
                    other => {
                        let name = su["tool_name"]
                            .as_str()
                            .or_else(|| su["tool_info"]["name"].as_str())
                            .or_else(|| su["name"].as_str())
                            .unwrap_or(other)
                            .to_string();
                        let params = if su["tool_info"]["parameters"].is_object() {
                            su["tool_info"]["parameters"].clone()
                        } else {
                            su["parameters"].clone()
                        };
                        let id = format!("agy-{}", su["step_index"].as_u64().unwrap_or(0));
                        let state = su["state"].as_str().unwrap_or("ACTIVE");
                        if !self.open_tools.contains_key(&id) && state == "ACTIVE" {
                            out.push(self.start_tool(id.clone(), &name, &params));
                        }
                        if state == "DONE" || state == "ERROR" {
                            if !self.open_tools.contains_key(&id) {
                                out.push(self.start_tool(id.clone(), &name, &params));
                            }
                            let err = (state == "ERROR")
                                .then(|| su["error"].as_str().unwrap_or("tool gagal").to_string());
                            if let Some(e) = self.finish_tool(&id, err, String::new()) {
                                out.push(e);
                            }
                        }
                    }
                }
            }
            "result" => {
                let r = &v["result"];
                let status = r["status"].as_str().unwrap_or("SUCCESS");
                if status.eq_ignore_ascii_case("SUCCESS") {
                    let usage = format_usage(&r["usage"], r["duration_seconds"].as_f64());
                    let resp = r["response"].as_str().map(str::to_string);
                    out.push(self.done(resp.as_deref(), usage));
                } else {
                    let detail = r["error"]
                        .as_str()
                        .or_else(|| r["response"].as_str())
                        .unwrap_or("");
                    out.push(self.error(format!("agy selesai dengan status {status}: {detail}")));
                }
            }
            "error" => {
                let msg = v["error"]
                    .as_str()
                    .or_else(|| v["message"].as_str())
                    .unwrap_or("error tak dikenal dari agy")
                    .to_string();
                out.push(self.error(msg));
            }
            _ => {}
        }
        out
    }

    fn feed_claude(&mut self, v: &Value) -> Vec<CliEvent> {
        let mut out = Vec::new();
        match v["type"].as_str().unwrap_or("") {
            "system" => {
                if v["subtype"] == "init" {
                    if let Some(id) = v["session_id"].as_str() {
                        out.push(CliEvent::Session(id.to_string()));
                    }
                }
            }
            "stream_event" => {
                self.saw_stream_event = true;
                let ev = &v["event"];
                match ev["type"].as_str().unwrap_or("") {
                    "content_block_delta" if ev["delta"]["type"] == "text_delta" => {
                        if let Some(t) = ev["delta"]["text"].as_str().filter(|t| !t.is_empty()) {
                            out.push(self.delta(t));
                        }
                    }
                    _ => {}
                }
            }
            "assistant" => {
                // Pesan asisten lengkap: argumen tool sudah utuh di sini.
                if let Some(blocks) = v["message"]["content"].as_array() {
                    for b in blocks {
                        match b["type"].as_str().unwrap_or("") {
                            "tool_use" => {
                                if let (Some(id), Some(name)) =
                                    (b["id"].as_str(), b["name"].as_str())
                                {
                                    if !self.open_tools.contains_key(id) {
                                        out.push(self.start_tool(
                                            id.to_string(),
                                            name,
                                            &b["input"],
                                        ));
                                    }
                                }
                            }
                            "text" if !self.saw_stream_event => {
                                if let Some(t) = b["text"].as_str().filter(|t| !t.is_empty()) {
                                    out.push(self.delta(t));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "user" => {
                if let Some(blocks) = v["message"]["content"].as_array() {
                    for b in blocks {
                        if b["type"] != "tool_result" {
                            continue;
                        }
                        let Some(id) = b["tool_use_id"].as_str() else {
                            continue;
                        };
                        let text = tool_result_text(&b["content"]);
                        let err = (b["is_error"] == true).then(|| ellipsize(&text, 300));
                        if let Some(e) = self.finish_tool(id, err, ellipsize(&text, 300)) {
                            out.push(e);
                        }
                    }
                }
            }
            "result" => {
                if v["is_error"] == true {
                    let msg = v["result"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| "claude mengembalikan error".to_string());
                    out.push(self.error(msg));
                } else {
                    let mut usage =
                        format_usage(&v["usage"], v["duration_ms"].as_f64().map(|ms| ms / 1000.0));
                    if let Some(cost) = v["total_cost_usd"].as_f64() {
                        let c = format!("${cost:.4}");
                        usage = Some(usage.map(|u| format!("{u} · {c}")).unwrap_or(c));
                    }
                    let r = v["result"].as_str().map(str::to_string);
                    out.push(self.done(r.as_deref(), usage));
                }
            }
            _ => {}
        }
        out
    }

    fn feed_gemini(&mut self, v: &Value) -> Vec<CliEvent> {
        let mut out = Vec::new();
        match v["type"].as_str().unwrap_or("") {
            "init" => {
                if let Some(id) = v["session_id"].as_str() {
                    out.push(CliEvent::Session(id.to_string()));
                }
            }
            "message" => {
                if v["role"] == "assistant" {
                    if let Some(c) = v["content"].as_str().filter(|c| !c.is_empty()) {
                        if v["delta"].as_bool().unwrap_or(true) || !self.saw_delta {
                            out.push(self.delta(c));
                        }
                    }
                }
            }
            "tool_use" | "tool_call" => {
                let name = v["tool_name"]
                    .as_str()
                    .or_else(|| v["name"].as_str())
                    .unwrap_or("tool");
                let params = if v["parameters"].is_object() {
                    &v["parameters"]
                } else {
                    &v["args"]
                };
                let id = v["tool_id"]
                    .as_str()
                    .or_else(|| v["id"].as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("gemini-{}", self.open_tools.len()));
                let params = params.clone();
                out.push(self.start_tool(id, name, &params));
            }
            "tool_result" => {
                let id = v["tool_id"]
                    .as_str()
                    .or_else(|| v["id"].as_str())
                    .unwrap_or("");
                let failed = v["status"]
                    .as_str()
                    .is_some_and(|s| !s.eq_ignore_ascii_case("success"));
                let text = v["output"].as_str().map(str::to_string).unwrap_or_default();
                let err = failed.then(|| {
                    v["error"]["message"]
                        .as_str()
                        .or_else(|| v["error"].as_str())
                        .map(|m| ellipsize(m, 300))
                        .unwrap_or_else(|| "tool gagal".to_string())
                });
                if let Some(e) = self.finish_tool(id, err, ellipsize(&text, 300)) {
                    out.push(e);
                }
            }
            "result" => {
                let status = v["status"].as_str().unwrap_or("success");
                if status.eq_ignore_ascii_case("success") {
                    let usage = format_usage(&v["stats"], None);
                    let r = v["response"].as_str().map(str::to_string);
                    out.push(self.done(r.as_deref(), usage));
                } else {
                    let d = v["error"]["message"]
                        .as_str()
                        .or_else(|| v["error"].as_str())
                        .unwrap_or("");
                    out.push(self.error(format!("gemini selesai dengan status {status}: {d}")));
                }
            }
            "error" => {
                let msg = v["message"]
                    .as_str()
                    .or_else(|| v["error"]["message"].as_str())
                    .unwrap_or("error tak dikenal dari gemini")
                    .to_string();
                out.push(self.error(msg));
            }
            _ => {}
        }
        out
    }
}

fn tool_result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

// ─── Proses ────────────────────────────────────────────────────────────────

/// Pegangan untuk menghentikan proses CLI (beserta anak-anaknya).
#[derive(Clone)]
pub struct CancelHandle {
    child: Arc<Mutex<Option<Child>>>,
    cancelled: Arc<AtomicBool>,
}

impl CancelHandle {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut g) = self.child.lock() {
            if let Some(child) = g.as_mut() {
                let pid = child.id();
                let _ = child.kill();
                // CLI biasanya punya anak (node, server MCP); grup proses = pid
                // karena `process_group(0)` saat spawn.
                #[cfg(unix)]
                {
                    let _ = Command::new("kill")
                        .args(["-TERM", "--", &format!("-{pid}")])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
                #[cfg(not(unix))]
                let _ = pid;
            }
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Direktori kerja kosong `~/.ducad/agent-workspace` (tool berkas milik CLI
/// tidak menyentuh proyek mana pun).
pub fn agent_workspace_dir() -> PathBuf {
    let d = home().join(".ducad").join("agent-workspace");
    if let Err(e) = std::fs::create_dir_all(&d) {
        log::warn!("cli agent: tidak bisa membuat {}: {e}", d.display());
    }
    d
}

/// Direktori yang dicari selain PATH (aplikasi dari Finder/Dock hanya
/// mewarisi PATH sistem minimal).
fn extra_bin_dirs() -> Vec<PathBuf> {
    let h = home();
    let mut dirs = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    for rel in [
        ".local/bin",
        ".antigravity/bin",
        ".claude/local",
        ".npm-global/bin",
        ".bun/bin",
        ".volta/bin",
        ".cargo/bin",
    ] {
        dirs.push(h.join(rel));
    }
    if let Ok(entries) = std::fs::read_dir(h.join(".nvm/versions/node")) {
        for e in entries.flatten() {
            dirs.push(e.path().join("bin"));
        }
    }
    dirs
}

/// PATH + [`extra_bin_dirs`], diwariskan ke proses CLI (butuh `node`, dll.).
pub fn augmented_path() -> std::ffi::OsString {
    let mut paths: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for d in extra_bin_dirs() {
        if d.is_dir() && !paths.contains(&d) {
            paths.push(d);
        }
    }
    std::env::join_paths(paths).unwrap_or_default()
}

/// Cari binary: path eksplisit → PATH → direktori umum → `$SHELL -lc command -v`.
pub fn resolve_binary(name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let direct = Path::new(name);
    if direct.components().count() > 1 {
        return direct.is_file().then(|| direct.to_path_buf());
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend(extra_bin_dirs());
    for d in dirs {
        let p = d.join(name);
        if p.is_file() {
            return Some(p);
        }
        #[cfg(windows)]
        for ext in ["exe", "cmd", "bat"] {
            let p = d.join(format!("{name}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    #[cfg(unix)]
    {
        // Nama sudah dibatasi karakter aman supaya tidak jadi injeksi shell.
        if name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            if let Ok(out) = Command::new(shell)
                .arg("-lc")
                .arg(format!("command -v {name}"))
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
            {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() && Path::new(&s).is_file() {
                    return Some(PathBuf::from(s));
                }
            }
        }
    }
    None
}

/// Lokasi `ducad-mcp`: di samping binary aplikasi, atau di PATH/`~/.cargo/bin`.
pub fn ducad_mcp_exe() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("ducad-mcp");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    resolve_binary("ducad-mcp")
}

/// Argumen server MCP DUCAD mode live.
pub fn mcp_server_args(socket: &Path) -> Vec<String> {
    vec![
        "--attach".into(),
        "--socket".into(),
        socket.to_string_lossy().to_string(),
    ]
}

/// Tulis `mcp-ducad.json` (format Claude Code / Cursor) ke workspace.
pub fn write_mcp_config(mcp_exe: &Path, socket: &Path) -> Result<PathBuf, String> {
    let body = json!({ "mcpServers": { MCP_SERVER_NAME: {
        "command": mcp_exe.to_string_lossy(),
        "args": mcp_server_args(socket),
    } } });
    let path = agent_workspace_dir().join("mcp-ducad.json");
    let text = serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?;
    std::fs::write(&path, text)
        .map_err(|e| format!("tidak bisa menulis {}: {e}", path.display()))?;
    Ok(path)
}

/// Ubah pesan "belum login" menjadi instruksi.
pub fn detect_login_problem(kind: CliAgentKind, output: &str) -> Option<String> {
    let lower = output.to_ascii_lowercase();
    let hit = [
        "not logged in",
        "not authenticated",
        "unauthenticated",
        "authentication required",
        "please log in",
        "please login",
        "please run /login",
        "login required",
        "invalid api key",
        "invalid authentication credentials",
        "status: 401",
        "\"code\": 401",
    ]
    .iter()
    .any(|p| lower.contains(p));
    if !hit {
        return None;
    }
    let bin = kind.default_binary();
    Some(match kind {
        CliAgentKind::Antigravity => format!("Antigravity belum login. Buka terminal, jalankan `{bin}` sekali dan masuk dengan akun Google, lalu coba lagi."),
        CliAgentKind::ClaudeCode => format!("Claude Code belum login. Buka terminal, jalankan `{bin}` dan selesaikan `/login`, lalu coba lagi."),
        CliAgentKind::GeminiCli => format!("Gemini CLI belum terautentikasi. Buka terminal, jalankan `{bin}` sekali dan masuk, lalu coba lagi."),
        CliAgentKind::Custom => "CLI melaporkan masalah autentikasi. Masuk dari terminal lalu coba lagi.".into(),
    })
}

fn tail(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.len() <= max {
        return s.to_string();
    }
    let start = (s.len() - max..s.len())
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(0);
    format!("…{}", &s[start..])
}

/// Jalankan CLI dan alirkan kejadiannya lewat receiver (dipoll UI dengan
/// `try_recv`). Proses hidup di thread latar.
pub fn spawn_stream(
    p: &CliAgentProfile,
    req: CliRequest,
) -> Result<(mpsc::Receiver<CliEvent>, CancelHandle), String> {
    let bin_name = p.effective_bin();
    if bin_name.is_empty() {
        return Err("Perintah CLI belum diisi (⚙ → CLI agent).".into());
    }
    let bin = resolve_binary(&bin_name).ok_or_else(|| {
        format!("CLI `{bin_name}` tidak ditemukan. Pasang dulu, atau isi path lengkapnya di ⚙ → CLI agent.")
    })?;
    std::fs::create_dir_all(&req.cwd)
        .map_err(|e| format!("tidak bisa membuat {}: {e}", req.cwd.display()))?;
    let args = build_args(p, &req);
    log::info!(
        "cli agent: {} ({:?}), {} argumen, cwd {}",
        bin.display(),
        p.kind,
        args.len(),
        req.cwd.display()
    );

    let mut cmd = Command::new(&bin);
    cmd.args(&args)
        .current_dir(&req.cwd)
        .env("PATH", augmented_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if p.kind == CliAgentKind::GeminiCli {
        // Mode headless menolak folder yang belum "dipercaya"; cwd adalah
        // folder kosong khusus DUCAD (`agent_workspace_dir`).
        cmd.env("GEMINI_CLI_TRUST_WORKSPACE", "true");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("gagal menjalankan `{}`: {e}", bin.display()))?;
    let stdout = child.stdout.take().ok_or("stdout CLI tidak tersedia")?;
    let stderr = child.stderr.take().ok_or("stderr CLI tidak tersedia")?;
    let (tx, rx) = mpsc::channel();
    let handle = CancelHandle {
        child: Arc::new(Mutex::new(Some(child))),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let err_buf = Arc::new(Mutex::new(String::new()));
    let err_thread = {
        let buf = Arc::clone(&err_buf);
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut s);
            if let Ok(mut g) = buf.lock() {
                *g = s;
            }
        })
    };
    let kind = p.kind;
    let h = handle.clone();
    std::thread::spawn(move || {
        let mut parser = StreamParser::new(kind);
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            for ev in parser.feed_line(&line) {
                if tx.send(ev).is_err() {
                    h.cancel();
                    return;
                }
            }
        }
        let status = h
            .child
            .lock()
            .ok()
            .and_then(|mut g| g.take())
            .and_then(|mut c| c.wait().ok());
        let _ = err_thread.join();
        if parser.is_finished() {
            return;
        }
        if h.is_cancelled() {
            let _ = tx.send(CliEvent::Chat(ChatEvent::Cancelled));
            return;
        }
        let stderr_text = err_buf.lock().map(|g| g.clone()).unwrap_or_default();
        let ok = status.map(|s| s.success()).unwrap_or(false);
        if ok {
            if let Some(ev) = parser.finish() {
                let _ = tx.send(ev);
                return;
            }
        }
        let msg = detect_login_problem(kind, &stderr_text).unwrap_or_else(|| {
            let code = status
                .and_then(|s| s.code())
                .map(|c| c.to_string())
                .unwrap_or_else(|| "sinyal".into());
            match tail(&stderr_text, 800) {
                d if d.is_empty() => format!("CLI berhenti dengan kode {code} tanpa hasil."),
                d => format!("CLI berhenti dengan kode {code}: {d}"),
            }
        });
        let _ = tx.send(CliEvent::Chat(ChatEvent::Error { message: msg }));
    });
    Ok((rx, handle))
}

/// Jalankan perintah singkat dan kembalikan stdout.
fn run_capture(bin: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(bin)
        .args(args)
        .env("PATH", augmented_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("gagal menjalankan `{}`: {e}", bin.display()))?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                return Err(format!(
                    "`{} {}` melewati batas {} s",
                    bin.display(),
                    args.join(" "),
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("gagal menunggu proses: {e}")),
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("gagal membaca keluaran: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(format!(
            "`{} {}` gagal ({}): {}",
            bin.display(),
            args.join(" "),
            out.status,
            tail(&format!("{stdout}\n{stderr}"), 600)
        ))
    }
}

/// Apakah keluaran `<cli> mcp list` menyebut server `ducad`.
pub fn mcp_list_mentions_ducad(output: &str) -> bool {
    output.lines().any(|l| {
        let first = l.split_whitespace().next().map(|w| w.trim_end_matches(':'));
        first == Some(MCP_SERVER_NAME) || l.contains(&format!("{MCP_SERVER_NAME}:"))
    })
}

/// Server `ducad` sudah terdaftar di konfigurasi global CLI?
pub fn check_mcp_registered(p: &CliAgentProfile) -> Result<bool, String> {
    let bin = resolve_binary(&p.effective_bin())
        .ok_or_else(|| format!("CLI `{}` tidak ditemukan", p.effective_bin()))?;
    let out = run_capture(&bin, &["mcp", "list"], Duration::from_secs(20))?;
    Ok(mcp_list_mentions_ducad(&out))
}

/// Argumen `<cli> mcp add` untuk server DUCAD.
pub fn register_args(kind: CliAgentKind, mcp_exe: &str, socket: &str) -> Option<Vec<String>> {
    let tail = vec![
        mcp_exe.to_string(),
        "--attach".into(),
        "--socket".into(),
        socket.to_string(),
    ];
    let mut a: Vec<String> = vec!["mcp".into(), "add".into()];
    match kind {
        CliAgentKind::Antigravity | CliAgentKind::ClaudeCode => {
            a.extend(
                ["--scope".into(), "user".into()]
                    .into_iter()
                    .filter(|_| kind == CliAgentKind::ClaudeCode),
            );
            a.extend([MCP_SERVER_NAME.to_string(), "--".into()]);
        }
        CliAgentKind::GeminiCli => {
            a.extend(["--scope".into(), "user".into(), MCP_SERVER_NAME.to_string()]);
        }
        CliAgentKind::Custom => return None,
    }
    a.extend(tail);
    Some(a)
}

/// Daftarkan server `ducad` (mode live) di konfigurasi global CLI.
pub fn register_mcp(p: &CliAgentProfile, mcp_exe: &Path, socket: &Path) -> Result<String, String> {
    let bin = resolve_binary(&p.effective_bin())
        .ok_or_else(|| format!("CLI `{}` tidak ditemukan", p.effective_bin()))?;
    let args = register_args(
        p.kind,
        &mcp_exe.to_string_lossy(),
        &socket.to_string_lossy(),
    )
    .ok_or("Untuk perintah kustom, daftarkan server MCP secara manual (lihat `{mcp_config}`).")?;
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_capture(&bin, &refs, Duration::from_secs(20))?;
    Ok(tail(&out, 300))
}

/// Uji cepat: binary ada, versi terbaca, dan prompt kecil dijawab.
pub fn test_connection(p: &CliAgentProfile) -> Result<String, String> {
    let bin = resolve_binary(&p.effective_bin()).ok_or_else(|| {
        format!(
            "CLI `{}` tidak ditemukan di PATH maupun lokasi instalasi umum.",
            p.effective_bin()
        )
    })?;
    let version = run_capture(&bin, &["--version"], Duration::from_secs(15))
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "(versi tidak diketahui)".into());
    let req = CliRequest {
        system_prompt: String::new(),
        user_prompt: "Reply with exactly the word OK and nothing else.".into(),
        session_id: None,
        cwd: agent_workspace_dir(),
        mcp_config: None,
    };
    let (rx, handle) = spawn_stream(p, req)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    let mut text = String::new();
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            handle.cancel();
            return Err(format!(
                "{} · {version}\nPrompt uji melewati 120 s.",
                bin.display()
            ));
        }
        match rx.recv_timeout(remaining) {
            Ok(CliEvent::Chat(ChatEvent::TextDelta { text: d })) => text.push_str(&d),
            Ok(CliEvent::Finished { text: t, usage }) => {
                let reply = if text.is_empty() { t } else { text };
                let usage = usage.map(|u| format!(" ({u})")).unwrap_or_default();
                return Ok(format!(
                    "{} · {version}\nBalasan: {}{usage}",
                    bin.display(),
                    reply.trim()
                ));
            }
            Ok(CliEvent::Chat(ChatEvent::Error { message })) => {
                return Err(format!("{} · {version}\n{message}", bin.display()))
            }
            Ok(_) => {}
            Err(_) => {
                handle.cancel();
                return Err(format!(
                    "{} · {version}\nCLI berhenti tanpa balasan.",
                    bin.display()
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> CliRequest {
        CliRequest {
            system_prompt: "SYS".into(),
            user_prompt: "USER".into(),
            session_id: Some("abc".into()),
            cwd: PathBuf::from("/tmp"),
            mcp_config: Some(PathBuf::from("/tmp/mcp.json")),
        }
    }

    fn feed(kind: CliAgentKind, lines: &str) -> Vec<CliEvent> {
        let mut p = StreamParser::new(kind);
        let mut out: Vec<CliEvent> = lines.lines().flat_map(|l| p.feed_line(l)).collect();
        out.extend(p.finish());
        out
    }

    #[test]
    fn split_args_handles_quotes() {
        assert_eq!(
            split_args(r#"--a "b c" 'd e' f\ g"#),
            vec!["--a", "b c", "d e", "f g"]
        );
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn claude_args_restrict_tools_to_ducad_mcp() {
        let mut p = CliAgentProfile::new(CliAgentKind::ClaudeCode);
        p.model = "opus".into();
        p.effort = "high".into();
        p.extra_args = "--max-turns 30".into();
        let a = build_args(&p, &req());
        let s = a.join(" ");
        assert!(
            s.contains("-p USER") && s.contains("--append-system-prompt SYS"),
            "{s}"
        );
        assert!(
            s.contains("--resume abc") && s.contains("--strict-mcp-config"),
            "{s}"
        );
        assert!(
            s.contains("--allowedTools mcp__ducad") && s.ends_with("--max-turns 30"),
            "{s}"
        );
    }

    #[test]
    fn agy_gemini_custom_args() {
        let mut p = CliAgentProfile::new(CliAgentKind::Antigravity);
        p.model = "gemini-3.8-flash-high".into();
        p.effort = "max".into();
        let a = build_args(&p, &req());
        assert!(a.contains(&"--conversation".to_string()));
        assert!(
            !a.contains(&"--effort".to_string()),
            "effort sudah di nama model"
        );
        assert!(a[1].starts_with("SYS") && a[1].ends_with("USER"));

        let g = build_args(&CliAgentProfile::new(CliAgentKind::GeminiCli), &req());
        let s = g.join(" ");
        assert!(
            s.contains("--allowed-mcp-server-names ducad") && s.contains("--approval-mode yolo"),
            "{s}"
        );

        let mut c = CliAgentProfile::new(CliAgentKind::Custom);
        c.extra_args = "mycli --cfg {mcp_config} --ask {prompt}".into();
        assert_eq!(
            build_args(&c, &req()),
            vec!["mycli", "--cfg", "/tmp/mcp.json", "--ask", "USER"]
        );
    }

    #[test]
    fn claude_stream_maps_text_tools_and_result() {
        let lines = r#"{"type":"system","subtype":"init","session_id":"s-1"}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Membuat "}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"blok."}}}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"mcp__ducad__run_ops","input":{"ops":[]}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"{\"committed\":true}"}]}]}}
{"type":"result","subtype":"success","is_error":false,"result":"Membuat blok.","usage":{"input_tokens":10,"output_tokens":5},"total_cost_usd":0.01}"#;
        let ev = feed(CliAgentKind::ClaudeCode, lines);
        assert_eq!(ev[0], CliEvent::Session("s-1".into()));
        assert!(ev.iter().any(
            |e| matches!(e, CliEvent::Chat(ChatEvent::ToolStart { name, .. }) if name == "run_ops")
        ));
        assert!(ev.iter().any(|e| matches!(e, CliEvent::Chat(ChatEvent::ToolDone { is_error: false, summary, .. }) if summary.contains("committed"))));
        let Some(CliEvent::Finished { text, usage }) = ev.last() else {
            panic!("{ev:?}")
        };
        assert_eq!(text, "Membuat blok.");
        assert!(usage.as_deref().unwrap_or("").contains("$0.0100"));
    }

    #[test]
    fn agy_and_gemini_streams() {
        let agy = r#"{"event":"init","conversation_id":"c-9"}
{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"mcp_tool","tool_info":{"name":"ducad/inspect","parameters":{}}}}
{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"mcp_tool","tool_info":{"name":"ducad/inspect","parameters":{}}}}
{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"Siap."}}
{"event":"result","result":{"status":"SUCCESS","response":"Siap.","duration_seconds":2.5}}"#;
        let ev = feed(CliAgentKind::Antigravity, agy);
        assert_eq!(ev[0], CliEvent::Session("c-9".into()));
        assert!(ev.iter().any(
            |e| matches!(e, CliEvent::Chat(ChatEvent::ToolDone { name, .. }) if name == "inspect")
        ));
        assert!(matches!(ev.last(), Some(CliEvent::Finished { text, .. }) if text == "Siap."));

        let gem = r#"{"type":"init","session_id":"g"}
{"type":"tool_use","tool_name":"run_ops","tool_id":"x","parameters":{"ops":[]}}
{"type":"tool_result","tool_id":"x","status":"error","error":{"message":"unknown_ref"}}
{"type":"result","status":"error","error":{"message":"kuota habis"}}"#;
        let ev = feed(CliAgentKind::GeminiCli, gem);
        assert!(ev.iter().any(|e| matches!(
            e,
            CliEvent::Chat(ChatEvent::ToolDone { is_error: true, .. })
        )));
        assert!(
            matches!(ev.last(), Some(CliEvent::Chat(ChatEvent::Error { message })) if message.contains("kuota"))
        );

        // Bentuk nyata Gemini CLI tanpa login (HTTP 401 di dalam pesan).
        let unauth = r#"{"type":"result","status":"error","error":{"type":"unknown","message":"[API Error: {\"error\":{\"code\": 401, \"message\": \"Request had invalid authentication credentials.\"}}]"}}"#;
        let ev = feed(CliAgentKind::GeminiCli, unauth);
        assert!(
            matches!(ev.last(), Some(CliEvent::Chat(ChatEvent::Error { message })) if message.contains("belum terautentikasi")),
            "{ev:?}"
        );
    }

    #[test]
    fn plain_text_fallback_login_and_names() {
        let ev = feed(CliAgentKind::ClaudeCode, "halo biasa");
        assert!(matches!(ev.last(), Some(CliEvent::Finished { text, .. }) if text == "halo biasa"));
        assert!(
            detect_login_problem(CliAgentKind::ClaudeCode, "Error: Not logged in")
                .unwrap()
                .contains("/login")
        );
        assert!(detect_login_problem(CliAgentKind::ClaudeCode, "boom").is_none());
        assert_eq!(short_tool_name("mcp__ducad__set_view"), "set_view");
        assert!(mcp_list_mentions_ducad(
            "ducad: /x/ducad-mcp --attach - ✓ Connected"
        ));
        assert!(!mcp_list_mentions_ducad("tabular: x"));
        let r = register_args(CliAgentKind::GeminiCli, "/b/ducad-mcp", "/s.sock").unwrap();
        assert_eq!(r[..5], ["mcp", "add", "--scope", "user", "ducad"]);
        let r = register_args(CliAgentKind::Antigravity, "/b/ducad-mcp", "/s.sock").unwrap();
        assert_eq!(r[..4], ["mcp", "add", "ducad", "--"]);
    }

    #[cfg(unix)]
    #[test]
    fn spawn_custom_script_streams_and_cancels() {
        let dir = std::env::temp_dir().join(format!("ducad-cli-agent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake.sh");
        std::fs::write(&script, "#!/bin/sh\necho baris satu\necho baris dua\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut p = CliAgentProfile::new(CliAgentKind::Custom);
        p.bin = script.to_string_lossy().to_string();
        p.extra_args = "{prompt}".into();
        let r = CliRequest {
            system_prompt: String::new(),
            user_prompt: "x".into(),
            session_id: None,
            cwd: dir.clone(),
            mcp_config: None,
        };
        let (rx, _h) = spawn_stream(&p, r.clone()).unwrap();
        let events: Vec<CliEvent> = rx.iter().collect();
        assert!(
            matches!(events.last(), Some(CliEvent::Finished { text, .. }) if text.contains("baris dua")),
            "{events:?}"
        );

        std::fs::write(&script, "#!/bin/sh\nsleep 30\n").unwrap();
        let (rx, h) = spawn_stream(&p, r).unwrap();
        h.cancel();
        let ev = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(ev, CliEvent::Chat(ChatEvent::Cancelled));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
