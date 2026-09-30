//! `ducad-cli chat` (P13.2): satu instruksi ke chat agent yang memakai
//! tool MCP DUCAD in-process. Teks model mengalir ke stderr; stdout berisi
//! laporan JSON akhir. Kunci API: `ANTHROPIC_API_KEY`/`OPENAI_API_KEY`
//! atau yang disimpan aplikasi (Keychain).

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use ducad_chat::{
    run_turn, secrets, ChatEvent, ChatSettings, ConvMessage, HttpModel, Policy, ProviderConfig,
    ToolDef, ToolExecutor, ToolOutput,
};
use ducad_mcp::server::{Server, INSTRUCTIONS};
use serde_json::{json, Value};

use super::print_json;
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    /// Part `.ducad` yang dibuka sebagai sesi `s1` (kosong = part baru).
    part: Option<PathBuf>,
    #[arg(long)]
    instruction: String,
    /// `anthropic`, `openai`, `ollama`, atau `saved` (pengaturan aplikasi).
    #[arg(long, default_value = "saved")]
    provider: String,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    base_url: Option<String>,
    /// Simpan sesi `s1` ke berkas ini setelah chat selesai.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Direktori root pagar path tool (default: direktori kerja).
    #[arg(long)]
    root: Option<PathBuf>,
    #[arg(long)]
    max_rounds: Option<usize>,
    /// Tulis setiap kejadian loop sebagai NDJSON ke stderr.
    #[arg(long)]
    events: bool,
}

/// Eksekutor: server MCP DUCAD in-process.
struct McpExecutor<'a>(&'a mut Server);

impl ToolExecutor for McpExecutor<'_> {
    fn call(&mut self, name: &str, input: Value) -> ToolOutput {
        let out = ducad_mcp::tools::call_out(self.0, name, input);
        ToolOutput {
            text: ducad_engine::tooling::compact_text(out.payload),
            image_png: out.image_png,
            is_error: out.is_error,
        }
    }
}

fn provider(a: &Args) -> Result<(ProviderConfig, usize), CliError> {
    let saved = ChatSettings::load(&ChatSettings::default_path());
    let mut p = match a.provider.as_str() {
        "saved" => saved.provider.clone(),
        "anthropic" => ProviderConfig::anthropic(),
        "openai" => ProviderConfig::openai(),
        "ollama" => ProviderConfig::ollama(),
        other => return Err(CliError::usage(format!("provider tidak dikenal: {other}"))),
    };
    if let Some(m) = &a.model {
        p.model = m.clone();
    }
    if let Some(u) = &a.base_url {
        p.base_url = u.clone();
    }
    secrets::load_into(&mut p);
    Ok((p, a.max_rounds.unwrap_or(saved.max_rounds)))
}

pub fn exec(a: Args) -> CliResult {
    let (cfg, max_rounds) = provider(&a)?;
    let root = match &a.root {
        Some(r) => r.clone(),
        None => std::env::current_dir().map_err(|e| CliError::usage(e.to_string()))?,
    };
    let mut server = Server::new(root).map_err(|e| CliError::usage(e.to_string()))?;
    let opened = match &a.part {
        Some(p) => ducad_mcp::tools::call_out(
            &mut server,
            "open_part",
            json!({ "path": p.to_string_lossy() }),
        ),
        None => ducad_mcp::tools::call_out(&mut server, "new_part", json!({})),
    };
    if opened.is_error {
        return Err(CliError::usage(opened.payload.to_string()));
    }
    let tools: Vec<ToolDef> = ducad_mcp::tools::chat_tools(false)
        .iter()
        .filter_map(ToolDef::from_mcp)
        .collect();
    let system = format!(
        "{}\n\n{INSTRUCTIONS}\n\nPart sudah terbuka sebagai sesi \"s1\".",
        ducad_chat::SYSTEM_PROMPT
    );
    let mut model = HttpModel::new(cfg);
    let mut conv = vec![ConvMessage::User {
        text: a.instruction.clone(),
    }];
    let policy = Policy {
        max_rounds,
        ..Default::default()
    };
    let mut tool_calls = Vec::new();
    let mut final_note = None;
    let mut error = None;
    let mut usage = ducad_chat::Usage::default();
    let events = a.events;
    let result = run_turn(
        &mut model,
        &mut McpExecutor(&mut server),
        &system,
        &tools,
        &mut conv,
        &policy,
        &mut |e| {
            if events {
                eprintln!("{}", serde_json::to_string(&e).unwrap_or_default());
            }
            match e {
                ChatEvent::TextDelta { text } if !events => eprint!("{text}"),
                ChatEvent::ToolStart { name, .. } if !events => eprintln!("\n· {name}"),
                ChatEvent::ToolDone {
                    name,
                    is_error,
                    summary,
                    ..
                } => {
                    tool_calls
                        .push(json!({ "name": name, "is_error": is_error, "summary": summary }));
                }
                ChatEvent::Usage { usage: u } => {
                    usage.input_tokens += u.input_tokens;
                    usage.output_tokens += u.output_tokens;
                    usage.cache_read_tokens += u.cache_read_tokens;
                }
                ChatEvent::Done { note, .. } => final_note = note,
                ChatEvent::Error { message } => error = Some(message),
                _ => {}
            }
        },
        &AtomicBool::new(false),
    );
    if !events {
        eprintln!();
    }
    let answer = conv
        .iter()
        .rev()
        .find_map(|m| match m {
            ConvMessage::Assistant { text, .. } if !text.trim().is_empty() => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let mut saved = Value::Null;
    if let Some(out) = &a.out {
        let r = ducad_mcp::tools::call_out(
            &mut server,
            "save_part",
            json!({ "session": "s1", "path": out.to_string_lossy() }),
        );
        if r.is_error {
            return Err(CliError::failed(r.payload.to_string()));
        }
        saved = r.payload;
    }
    print_json(&json!({
        "model": model.config().label(),
        "answer": answer,
        "tool_calls": tool_calls,
        "usage": usage,
        "note": final_note,
        "error": error,
        "saved": saved,
    }))?;
    Ok(if result.is_err() {
        Exit::OpFailed
    } else {
        Exit::Ok
    })
}
