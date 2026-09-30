//! Chat agent DUCAD (P13.1).
//!
//! Crate ini berisi semua yang dibutuhkan untuk mengobrol dengan model
//! bahasa yang bisa memanggil tool DUCAD, tanpa GUI dan tanpa kernel:
//!
//! - [`ChatModel`]: satu giliran model (HTTP Anthropic / OpenAI-compatible
//!   di [`http`], balasan terskrip di [`scripted`] untuk tes).
//! - [`ToolExecutor`]: pelaksana tool. CLI memakai server MCP in-process;
//!   GUI mengirim permintaan ke jembatan agent di UI thread.
//! - [`run_turn`]: loop tool-use (maks [`Policy::max_rounds`] putaran).
//!
//! Definisi tool TIDAK ditulis di sini: pemanggil mengambilnya dari
//! `ducad_mcp::tools::definitions()` sehingga agent di dalam aplikasi dan
//! agent eksternal lewat MCP melihat tool yang persis sama.

pub mod anthropic;
pub mod config;
pub mod http;
pub mod openai;
pub mod scripted;
pub mod secrets;
pub mod sse;
mod turn;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use config::{ApiStyle, ChatSettings, ProviderConfig};
pub use http::HttpModel;
pub use scripted::ScriptedModel;
pub use turn::{run_turn, Policy, SYSTEM_PROMPT};

/// Batas teks hasil tool yang diumpankan ke model (pola TABULAR).
pub const MAX_TOOL_RESULT_BYTES: usize = 20 * 1024;

/// Definisi tool dalam bentuk netral provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl ToolDef {
    /// Dari entri `tools/list` MCP (`name`, `description`, `inputSchema`).
    pub fn from_mcp(v: &Value) -> Option<Self> {
        Some(Self {
            name: v.get("name")?.as_str()?.to_string(),
            description: v
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            input_schema: v.get("inputSchema")?.clone(),
        })
    }
}

/// Satu panggilan tool dari model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
    /// Argumen tidak bisa diurai (JSON rusak / terpotong). Tool tidak
    /// dijalankan; model menerima error ini sebagai hasilnya.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_error: Option<String>,
}

/// Hasil satu tool.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub text: String,
    /// PNG (mis. `render_view`). Tidak disimpan di riwayat.
    #[serde(skip)]
    pub image_png: Option<Vec<u8>>,
    #[serde(default)]
    pub is_error: bool,
}

impl ToolOutput {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            image_png: None,
            is_error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            image_png: None,
            is_error: true,
        }
    }
}

/// Hasil tool yang ditautkan ke panggilannya.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub name: String,
    pub output: ToolOutput,
}

/// Pesan percakapan netral provider (pola `ConvMessage` TABULAR).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ConvMessage {
    User {
        text: String,
    },
    Assistant {
        text: String,
        #[serde(default)]
        calls: Vec<ToolCall>,
        /// Blok konten asli Anthropic (termasuk blok thinking bertanda
        /// tangan) yang WAJIB dikirim ulang apa adanya.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        raw: Option<Value>,
    },
    ToolResults {
        results: Vec<ToolResult>,
    },
}

/// Alasan model berhenti.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    #[default]
    EndTurn,
    ToolUse,
    MaxTokens,
    Refusal,
    Other(String),
}

impl StopReason {
    pub fn parse(s: &str) -> Self {
        match s {
            "end_turn" | "stop" | "stop_sequence" => Self::EndTurn,
            "tool_use" | "tool_calls" | "function_call" => Self::ToolUse,
            "max_tokens" | "length" => Self::MaxTokens,
            "refusal" | "content_filter" => Self::Refusal,
            other => Self::Other(other.to_string()),
        }
    }
}

/// Pemakaian token satu giliran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
}

/// Balasan satu giliran model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelTurn {
    pub text: String,
    pub calls: Vec<ToolCall>,
    pub raw: Option<Value>,
    pub stop: StopReason,
    pub usage: Usage,
}

/// Masukan satu giliran.
pub struct TurnRequest<'a> {
    pub system: &'a str,
    pub tools: &'a [ToolDef],
    pub conv: &'a [ConvMessage],
}

/// Model bahasa. `turn` boleh lambat (detik–menit): panggil dari thread
/// latar, jangan dari UI thread.
pub trait ChatModel: Send {
    fn name(&self) -> String;
    fn turn(
        &mut self,
        req: &TurnRequest,
        on_text: &mut dyn FnMut(&str),
        cancel: &std::sync::atomic::AtomicBool,
    ) -> anyhow::Result<ModelTurn>;
}

/// Pelaksana tool. Error tool dikembalikan sebagai `ToolOutput::error`,
/// bukan `Err`, supaya model bisa memperbaikinya.
pub trait ToolExecutor {
    fn call(&mut self, name: &str, input: Value) -> ToolOutput;
}

/// Kejadian loop untuk UI/CLI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ChatEvent {
    TextDelta {
        text: String,
    },
    ToolStart {
        id: String,
        name: String,
        input: Value,
    },
    ToolDone {
        id: String,
        name: String,
        summary: String,
        is_error: bool,
        #[serde(skip)]
        image_png: Option<Vec<u8>>,
    },
    Usage {
        usage: Usage,
    },
    Done {
        rounds: usize,
        stop: StopReason,
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    Cancelled,
    Error {
        message: String,
    },
}

/// Potong teks pada batas karakter, beri penanda bila terpotong.
pub fn truncate_text(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let cut = (0..=max_bytes)
        .rev()
        .find(|k| text.is_char_boundary(*k))
        .unwrap_or(0);
    format!(
        "{}\n…[dipotong: {} dari {} byte]",
        &text[..cut],
        cut,
        text.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_has_no_gui_or_kernel_dependency() {
        let manifest = include_str!("../Cargo.toml");
        let deps = manifest.split("[dependencies]").nth(1).unwrap_or_default();
        for bad in [
            "egui",
            "eframe",
            "wgpu",
            "ducad-kernel",
            "ducad-render",
            "rfd",
        ] {
            assert!(
                !deps.contains(bad),
                "ducad-chat tidak boleh bergantung pada {bad}"
            );
        }
    }

    #[test]
    fn tooldef_from_mcp_and_truncate() {
        let v =
            serde_json::json!({"name":"inspect","description":"d","inputSchema":{"type":"object"}});
        let t = ToolDef::from_mcp(&v).unwrap();
        assert_eq!(t.name, "inspect");
        assert!(ToolDef::from_mcp(&serde_json::json!({"name":"x"})).is_none());
        let s = "é".repeat(100);
        let out = truncate_text(&s, 11);
        assert!(out.starts_with("ééééé"));
        assert!(out.contains("dipotong"));
        assert_eq!(truncate_text("abc", 10), "abc");
    }

    #[test]
    fn conv_roundtrips_json() {
        let conv = vec![
            ConvMessage::User { text: "hai".into() },
            ConvMessage::Assistant {
                text: "ok".into(),
                calls: vec![ToolCall {
                    id: "t1".into(),
                    name: "inspect".into(),
                    input: serde_json::json!({}),
                    input_error: None,
                }],
                raw: None,
            },
            ConvMessage::ToolResults {
                results: vec![ToolResult {
                    call_id: "t1".into(),
                    name: "inspect".into(),
                    output: ToolOutput::ok("{}"),
                }],
            },
        ];
        let s = serde_json::to_string(&conv).unwrap();
        let back: Vec<ConvMessage> = serde_json::from_str(&s).unwrap();
        assert_eq!(back, conv);
        assert_eq!(StopReason::parse("tool_calls"), StopReason::ToolUse);
        assert_eq!(StopReason::parse("length"), StopReason::MaxTokens);
    }
}
