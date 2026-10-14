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

// Prinsip "jangan pernah crash" (ADR 0009): tidak ada unwrap/expect/panic di
// kode produksi; modul tes dikecualikan.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
pub mod anthropic;
// Harness CLI agent butuh proses anak + soket: tidak ada di tablet.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub mod cli;
pub mod config;
pub mod http;
pub mod openai;
pub mod scripted;
pub mod secrets;
pub mod sse;
mod turn;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use config::{ApiStyle, ChatBackend, ChatSettings, CliProfileData, ProviderConfig, CLI_KINDS};
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

/// Batas ukuran satu gambar lampiran (batas API Anthropic: 5 MB).
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// Gambar yang dilampirkan pengguna ke pesannya (W6): foto sketsa tangan,
/// tangkapan gambar lama, foto part. Jenisnya dikenali dari byte awal,
/// bukan dari ekstensi berkas.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatImage {
    /// `image/png`, `image/jpeg`, `image/gif`, atau `image/webp`.
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

impl ChatImage {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        let media_type = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png"
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            "image/jpeg"
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            "image/gif"
        } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            "image/webp"
        } else {
            return Err("bukan gambar PNG, JPEG, GIF, atau WebP".to_string());
        };
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(format!(
                "gambar {:.1} MB melebihi batas {} MB",
                bytes.len() as f64 / (1024.0 * 1024.0),
                MAX_IMAGE_BYTES / (1024 * 1024)
            ));
        }
        Ok(Self { media_type, bytes })
    }

    pub fn from_path(path: &std::path::Path) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("gagal membaca {}: {e}", path.display()))?;
        Self::from_bytes(bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn base64(&self) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&self.bytes)
    }
}

/// Pesan percakapan netral provider (pola `ConvMessage` TABULAR).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ConvMessage {
    User {
        text: String,
        /// Gambar lampiran. Tidak disimpan di riwayat (seperti gambar hasil
        /// tool); hanya gambar pesan pengguna TERAKHIR yang dikirim ulang.
        #[serde(skip)]
        images: Vec<ChatImage>,
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

impl ConvMessage {
    /// Pesan pengguna tanpa lampiran.
    pub fn user(text: impl Into<String>) -> Self {
        ConvMessage::User {
            text: text.into(),
            images: Vec::new(),
        }
    }
}

/// Indeks pesan pengguna terakhir yang membawa gambar.
pub fn last_user_images(conv: &[ConvMessage]) -> Option<usize> {
    conv.iter()
        .rposition(|m| matches!(m, ConvMessage::User { images, .. } if !images.is_empty()))
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
            ConvMessage::User {
            text: "hai".into(),
            images: Vec::new(),
        },
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

#[cfg(test)]
mod image_tests {
    use super::*;
    use serde_json::json;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2];

    #[test]
    fn image_type_comes_from_the_bytes() {
        assert_eq!(ChatImage::from_bytes(PNG.to_vec()).unwrap().media_type, "image/png");
        assert_eq!(ChatImage::from_bytes(JPEG.to_vec()).unwrap().media_type, "image/jpeg");
        assert!(ChatImage::from_bytes(b"%PDF-1.4".to_vec()).is_err());
        let mut big = PNG.to_vec();
        big.resize(MAX_IMAGE_BYTES + 1, 0);
        assert!(ChatImage::from_bytes(big).unwrap_err().contains("batas"));
    }

    fn conv() -> Vec<ConvMessage> {
        let img = |b: &[u8]| ChatImage::from_bytes(b.to_vec()).unwrap();
        vec![
            ConvMessage::User {
                text: "ini sketsanya".into(),
                images: vec![img(PNG)],
            },
            ConvMessage::Assistant {
                text: "ok".into(),
                calls: vec![],
                raw: None,
            },
            ConvMessage::User {
                text: "dan foto part lawannya".into(),
                images: vec![img(JPEG), img(PNG)],
            },
            ConvMessage::user("lanjut"),
        ]
    }

    #[test]
    fn only_the_latest_user_images_are_sent_to_anthropic() {
        let msgs = anthropic::messages(&conv());
        // Pesan lama: gambarnya diganti penanda teks.
        assert_eq!(msgs[0]["content"][0]["type"], "text");
        assert_eq!(msgs[0]["content"][1]["text"], "ini sketsanya");
        // Pesan terakhir yang bergambar: gambar mendahului teks.
        let latest = &msgs[2]["content"];
        assert_eq!(latest[0]["type"], "image");
        assert_eq!(latest[0]["source"]["media_type"], "image/jpeg");
        assert_eq!(latest[1]["source"]["media_type"], "image/png");
        assert_eq!(latest[2]["text"], "dan foto part lawannya");
        assert!(!latest[0]["source"]["data"].as_str().unwrap().is_empty());
    }

    #[test]
    fn openai_gets_data_urls_for_the_latest_images() {
        let msgs = openai::messages(&conv());
        let users: Vec<&serde_json::Value> =
            msgs.iter().filter(|m| m["role"] == "user").collect();
        assert_eq!(users[0]["content"], json!("ini sketsanya"));
        let parts = users[1]["content"].as_array().unwrap();
        assert_eq!(parts[0]["text"], "dan foto part lawannya");
        assert!(parts[1]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        assert_eq!(parts.len(), 3);
        assert_eq!(users[2]["content"], json!("lanjut"));
    }

    #[test]
    fn images_are_not_written_to_history() {
        let text = serde_json::to_string(&conv()[2]).unwrap();
        assert!(!text.contains("images"), "{text}");
        let back: ConvMessage = serde_json::from_str(&text).unwrap();
        assert_eq!(back, ConvMessage::user("dan foto part lawannya"));
    }
}
