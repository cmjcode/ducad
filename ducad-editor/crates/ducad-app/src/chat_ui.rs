//! Chat AI di aplikasi (P13.3).
//!
//! Loop agent berjalan di thread latar ([`ducad_chat::run_turn`]). Setiap
//! tool yang dipanggil model dikirim sebagai `BridgeRequest` ke kanal
//! in-process jembatan agent, lalu dieksekusi di UI thread oleh kode yang
//! SAMA dengan `ducad-mcp --attach` (`agent_bridge.rs`): satu batch = satu
//! langkah undo, `propose_ops` menampilkan ghost dan menunggu Terima/Tolak,
//! `accept_proposal` tidak tersedia bagi agent.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use ducad_chat::{
    run_turn, secrets, ChatEvent, ChatModel, ChatSettings, ConvMessage, HttpModel, Policy,
    ProviderConfig, ToolDef, ToolExecutor, ToolOutput, Usage,
};
use ducad_ui::{ChatItem, ChatPanel, ChatPanelEvent, ChatPanelState, ChatProviderForm, ChatRole};
use serde_json::Value;

use crate::agent_bridge::{BridgeRequest, REPLY_TIMEOUT_SECS};
use crate::app::DuCADApp;
use crate::chat_history::ChatHistory;

/// Awalan baris konteks yang disisipkan di pesan user (dibuang saat
/// transkrip dibangun ulang dari riwayat).
const CONTEXT_PREFIX: &str = "[Konteks DUCAD: ";

/// Pesan dari thread chat.
enum ChatMsg {
    Event(ChatEvent),
    Finished(Vec<ConvMessage>),
}

#[derive(Default)]
pub struct ChatState {
    pub panel: ChatPanelState,
    pub settings: ChatSettings,
    conv: Vec<ConvMessage>,
    rx: Option<Receiver<ChatMsg>>,
    cancel: Option<Arc<AtomicBool>>,
    session_id: Option<i64>,
    history: Option<ChatHistory>,
    loaded: bool,
    /// Indeks item asisten yang sedang dialiri teks.
    streaming: Option<usize>,
    /// id panggilan tool → indeks item kartunya.
    tool_items: HashMap<String, usize>,
    usage: Usage,
}

/// Eksekutor tool: kirim ke UI thread lewat jembatan, tunggu balasan.
struct BridgeExecutor {
    tx: Sender<BridgeRequest>,
    ctx: egui::Context,
    next_id: u64,
    cancel: Arc<AtomicBool>,
}

/// Ubah balasan jembatan (`{payload, image_png, is_error}`) jadi hasil tool.
fn output_from_reply(v: &Value) -> ToolOutput {
    let payload = v.get("payload").cloned().unwrap_or(Value::Null);
    ToolOutput {
        text: ducad_engine::tooling::compact_text(payload),
        image_png: v
            .get("image_png")
            .and_then(Value::as_str)
            .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok()),
        is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(false),
    }
}

impl ToolExecutor for BridgeExecutor {
    fn call(&mut self, name: &str, input: Value) -> ToolOutput {
        self.next_id += 1;
        let (reply, rx) = mpsc::channel();
        let req = BridgeRequest {
            // Rentang id terpisah dari klien soket supaya log mudah dibaca.
            id: 1_000_000 + self.next_id,
            method: name.to_string(),
            params: input,
            reply,
        };
        if self.tx.send(req).is_err() {
            return ToolOutput::error("aplikasi tidak lagi menerima permintaan");
        }
        self.ctx.request_repaint();
        let deadline = Instant::now() + Duration::from_secs(REPLY_TIMEOUT_SECS + 10);
        loop {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(v) => return output_from_reply(&v),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if self.cancel.load(Ordering::Relaxed) {
                        return ToolOutput::error("dibatalkan pengguna");
                    }
                    if Instant::now() >= deadline {
                        return ToolOutput::error("aplikasi tidak menjawab dalam batas waktu");
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return ToolOutput::error("aplikasi menutup permintaan tanpa balasan");
                }
            }
        }
    }
}

/// Definisi tool mode live (sama dengan `ducad-mcp --attach`).
pub fn live_tools() -> Vec<ToolDef> {
    ducad_mcp::tools::chat_tools(true)
        .iter()
        .filter_map(ToolDef::from_mcp)
        .collect()
}

/// Prompt sistem chat di aplikasi (stabil sepanjang percakapan → cache).
pub fn system_prompt() -> String {
    format!(
        "{}\n\n{}{}",
        ducad_chat::SYSTEM_PROMPT,
        ducad_mcp::server::INSTRUCTIONS,
        ducad_mcp::server::ATTACH_INSTRUCTIONS
    )
}

fn preset_config(i: usize, current: &ProviderConfig) -> ProviderConfig {
    match i {
        0 => ProviderConfig::anthropic(),
        1 => ProviderConfig::openai(),
        2 => ProviderConfig::ollama(),
        _ => ProviderConfig {
            style: ducad_chat::ApiStyle::OpenaiCompatible,
            ..current.clone()
        },
    }
}

fn preset_index(p: &ProviderConfig) -> usize {
    match (p.style, p.host()) {
        (ducad_chat::ApiStyle::Anthropic, _) => 0,
        (_, "api.openai.com") => 1,
        _ if p.is_local() => 2,
        _ => 3,
    }
}

fn strip_context(text: &str) -> &str {
    match text.strip_prefix(CONTEXT_PREFIX) {
        Some(rest) => rest.split_once('\n').map(|(_, t)| t).unwrap_or(rest),
        None => text,
    }
}

/// Bangun ulang transkrip tampilan dari percakapan tersimpan.
fn items_from_conv(conv: &[ConvMessage]) -> Vec<ChatItem> {
    let mut items = Vec::new();
    for m in conv {
        match m {
            ConvMessage::User { text } => {
                items.push(ChatItem::text(ChatRole::User, strip_context(text)))
            }
            ConvMessage::Assistant { text, calls, .. } => {
                if !text.trim().is_empty() {
                    items.push(ChatItem::text(ChatRole::Assistant, text.clone()));
                }
                for c in calls {
                    let mut it = ChatItem::text(ChatRole::Tool, "");
                    it.tool = c.name.clone();
                    it.detail = ducad_chat::truncate_text(&c.input.to_string(), 2000);
                    it.ok = Some(true);
                    items.push(it);
                }
            }
            ConvMessage::ToolResults { results } => {
                for r in results {
                    if let Some(it) = items
                        .iter_mut()
                        .rev()
                        .find(|i| i.role == ChatRole::Tool && i.tool == r.name && i.text.is_empty())
                    {
                        it.ok = Some(!r.output.is_error);
                        it.text = ducad_chat::truncate_text(&r.output.text.replace('\n', " "), 300);
                    }
                }
            }
        }
    }
    items
}

fn texture_from_png(ctx: &egui::Context, png: &[u8], name: &str) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(name, color, egui::TextureOptions::LINEAR))
}

impl ChatState {
    pub(crate) fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        self.settings = ChatSettings::load(&ChatSettings::default_path());
        self.form_from_settings();
    }

    fn form_from_settings(&mut self) {
        let p = &self.settings.provider;
        self.panel.form = ChatProviderForm {
            preset: preset_index(p),
            base_url: p.base_url.clone(),
            model: p.model.clone(),
            api_key_input: String::new(),
            key_saved: secrets::has_stored(p),
            confirm_writes: self.settings.confirm_writes,
            allow_external: self.settings.allow_external,
        };
        self.panel.provider_label = p.label();
    }

    fn history(&mut self) -> &ChatHistory {
        self.history.get_or_insert_with(ChatHistory::open_default)
    }

    fn push(&mut self, item: ChatItem) {
        self.panel.items.push(item);
        self.panel.scroll_to_bottom = true;
    }

    fn apply_event(&mut self, ctx: &egui::Context, e: ChatEvent) {
        match e {
            ChatEvent::TextDelta { text } => match self.streaming {
                Some(i) if i < self.panel.items.len() => self.panel.items[i].text.push_str(&text),
                _ => {
                    self.push(ChatItem::text(ChatRole::Assistant, text));
                    self.streaming = Some(self.panel.items.len() - 1);
                }
            },
            ChatEvent::ToolStart { id, name, input } => {
                self.streaming = None;
                let mut it = ChatItem::text(ChatRole::Tool, "");
                it.tool = name;
                it.detail = ducad_chat::truncate_text(&input.to_string(), 2000);
                self.push(it);
                self.tool_items.insert(id, self.panel.items.len() - 1);
            }
            ChatEvent::ToolDone {
                id,
                name,
                summary,
                is_error,
                image_png,
            } => {
                self.streaming = None;
                let idx = match self.tool_items.get(&id) {
                    Some(i) => *i,
                    None => {
                        let mut it = ChatItem::text(ChatRole::Tool, "");
                        it.tool = name;
                        self.push(it);
                        self.panel.items.len() - 1
                    }
                };
                if let Some(it) = self.panel.items.get_mut(idx) {
                    it.ok = Some(!is_error);
                    it.text = summary;
                    if let Some(png) = image_png {
                        it.image = texture_from_png(ctx, &png, &format!("chat-tool-{id}"));
                    }
                }
                self.panel.scroll_to_bottom = true;
            }
            ChatEvent::Usage { usage } => {
                self.usage.input_tokens += usage.input_tokens;
                self.usage.output_tokens += usage.output_tokens;
                self.usage.cache_read_tokens += usage.cache_read_tokens;
                let (i, o, c) = (
                    self.usage.input_tokens.to_string(),
                    self.usage.output_tokens.to_string(),
                    self.usage.cache_read_tokens.to_string(),
                );
                self.panel.usage_label = ducad_i18n::t!(
                    "chat-usage",
                    input = i.as_str(),
                    output = o.as_str(),
                    cached = c.as_str()
                );
            }
            ChatEvent::Done { note, .. } => {
                if let Some(n) = note {
                    self.push(ChatItem::text(ChatRole::Notice, n));
                }
            }
            ChatEvent::Cancelled => self.push(ChatItem::text(
                ChatRole::Notice,
                ducad_i18n::t!("chat-cancelled"),
            )),
            ChatEvent::Error { message } => self.push(ChatItem::text(ChatRole::Error, message)),
        }
    }

    fn save_history(&mut self) {
        if self.conv.is_empty() {
            return;
        }
        let title = self
            .conv
            .iter()
            .find_map(|m| match m {
                ConvMessage::User { text } => {
                    Some(strip_context(text).chars().take(60).collect::<String>())
                }
                _ => None,
            })
            .unwrap_or_default();
        let id = self.session_id;
        let conv = std::mem::take(&mut self.conv);
        self.session_id = self.history().save(id, &title, &conv);
        self.conv = conv;
    }
}

impl DuCADApp {
    /// Buka panel chat AI.
    pub fn open_chat(&mut self) {
        self.chat.ensure_loaded();
        self.chat.panel.open = true;
        self.sync_ai_privacy();
    }

    /// Samakan kebijakan privasi aplikasi dengan pengaturan chat.
    pub(crate) fn sync_ai_privacy(&mut self) {
        self.ai.privacy = if self.chat.settings.allow_external {
            crate::assist_ui::AiPrivacy::AllowExternal
        } else {
            crate::assist_ui::AiPrivacy::OfflineOnly
        };
    }

    /// Ringkasan dokumen yang disisipkan di depan pesan user.
    fn chat_context(&self) -> String {
        let file = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "belum disimpan".into());
        let bodies: Vec<String> = self
            .native_body_refs()
            .into_iter()
            .map(|(name, visible, _, _)| {
                if visible {
                    name.to_string()
                } else {
                    format!("{name} (tersembunyi)")
                }
            })
            .collect();
        let params = serde_json::to_string(&self.agent_meta.design.params).unwrap_or_default();
        format!(
            "{CONTEXT_PREFIX}berkas {file}; {} body [{}]; params {params}]",
            bodies.len(),
            bodies.join(", ")
        )
    }

    /// Kirim isi kotak input dengan model dari pengaturan.
    fn chat_send(&mut self, ctx: &egui::Context) {
        let mut cfg = self.chat.settings.provider.clone();
        if !cfg.is_local() && !self.chat.settings.allow_external {
            let host = cfg.host().to_string();
            self.chat.push(ChatItem::text(
                ChatRole::Error,
                ducad_i18n::t!("chat-blocked-privacy", host = host.as_str()),
            ));
            return;
        }
        secrets::load_into(&mut cfg);
        self.chat_start(ctx, Box::new(HttpModel::new(cfg)));
    }

    /// Mulai satu giliran dengan `model` (dipisah supaya tes bisa memakai
    /// model terskrip).
    pub(crate) fn chat_start(&mut self, ctx: &egui::Context, mut model: Box<dyn ChatModel>) {
        let text = self.chat.panel.input.trim().to_string();
        if text.is_empty() || self.chat.rx.is_some() {
            return;
        }
        self.chat.panel.input.clear();
        self.chat.push(ChatItem::text(ChatRole::User, text.clone()));
        self.chat.streaming = None;
        let mut conv = self.chat.conv.clone();
        conv.push(ConvMessage::User {
            text: format!("{}\n{text}", self.chat_context()),
        });
        let tools = live_tools();
        let system = system_prompt();
        let policy = Policy {
            max_rounds: self.chat.settings.max_rounds,
            confirm_writes: self.chat.settings.confirm_writes,
            ..Default::default()
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let mut exec = BridgeExecutor {
            tx: self.bridge.local_sender(),
            ctx: ctx.clone(),
            next_id: 0,
            cancel: cancel.clone(),
        };
        let (tx, rx) = mpsc::channel();
        let ctx2 = ctx.clone();
        let cancel2 = cancel.clone();
        std::thread::spawn(move || {
            let mut emit = |e: ChatEvent| {
                let _ = tx.send(ChatMsg::Event(e));
                ctx2.request_repaint();
            };
            let _ = run_turn(
                model.as_mut(),
                &mut exec,
                &system,
                &tools,
                &mut conv,
                &policy,
                &mut emit,
                &cancel2,
            );
            let _ = tx.send(ChatMsg::Finished(conv));
            ctx2.request_repaint();
        });
        self.chat.rx = Some(rx);
        self.chat.cancel = Some(cancel);
        self.chat.panel.busy = true;
    }

    /// Terima kejadian dari thread chat (dipanggil tiap frame).
    pub(crate) fn chat_poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.chat.rx.take() else {
            return;
        };
        let mut finished = None;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ChatMsg::Event(e) => self.chat.apply_event(ctx, e),
                ChatMsg::Finished(conv) => finished = Some(conv),
            }
        }
        match finished {
            Some(conv) => {
                self.chat.conv = conv;
                self.chat.panel.busy = false;
                self.chat.cancel = None;
                self.chat.streaming = None;
                self.chat.save_history();
            }
            None => {
                self.chat.rx = Some(rx);
                ctx.request_repaint_after(Duration::from_millis(40));
            }
        }
    }

    fn chat_save_settings(&mut self) {
        let f = self.chat.panel.form.clone();
        let mut p = preset_config(f.preset, &self.chat.settings.provider);
        p.base_url = f.base_url.trim().to_string();
        p.model = f.model.trim().to_string();
        if !f.api_key_input.trim().is_empty() {
            if let Err(e) = secrets::save(&p, &f.api_key_input) {
                self.chat
                    .push(ChatItem::text(ChatRole::Error, format!("{e:#}")));
            }
        }
        self.chat.settings.provider = p;
        self.chat.settings.confirm_writes = f.confirm_writes;
        self.chat.settings.allow_external = f.allow_external;
        match self.chat.settings.save(&ChatSettings::default_path()) {
            Ok(()) => self.chat.push(ChatItem::text(
                ChatRole::Notice,
                ducad_i18n::t!("chat-saved"),
            )),
            Err(e) => self
                .chat
                .push(ChatItem::text(ChatRole::Error, format!("{e:#}"))),
        }
        self.sync_ai_privacy();
        if !f.allow_external && self.bridge.enabled {
            self.bridge.shutdown();
            self.model_status = Some(ducad_i18n::t!("bridge-off"));
        }
        self.chat.form_from_settings();
        self.chat.panel.settings_open = false;
    }

    /// Terima kejadian latar lalu render panel.
    pub fn chat_frame(&mut self, ctx: &egui::Context) {
        self.chat_poll(ctx);
        let Some(event) = ChatPanel::show(ctx, &mut self.chat.panel) else {
            return;
        };
        match event {
            ChatPanelEvent::Send => self.chat_send(ctx),
            ChatPanelEvent::Stop => {
                if let Some(c) = &self.chat.cancel {
                    c.store(true, Ordering::Relaxed);
                }
            }
            ChatPanelEvent::NewChat => {
                self.chat.conv.clear();
                self.chat.panel.items.clear();
                self.chat.tool_items.clear();
                self.chat.session_id = None;
                self.chat.usage = Usage::default();
                self.chat.panel.usage_label.clear();
            }
            ChatPanelEvent::SaveSettings => self.chat_save_settings(),
            ChatPanelEvent::PresetChanged(i) => {
                let p = preset_config(i, &self.chat.settings.provider);
                self.chat.panel.form.base_url = p.base_url.clone();
                self.chat.panel.form.model = p.model.clone();
                self.chat.panel.form.key_saved = secrets::has_stored(&p);
            }
            ChatPanelEvent::OpenHistory => {
                self.chat.panel.sessions = self.chat.history().list();
            }
            ChatPanelEvent::LoadSession(id) => {
                if self.chat.panel.busy {
                    return;
                }
                if let Some(conv) = self.chat.history().load(id) {
                    self.chat.panel.items = items_from_conv(&conv);
                    self.chat.conv = conv;
                    self.chat.session_id = Some(id);
                    self.chat.tool_items.clear();
                    self.chat.panel.history_open = false;
                    self.chat.panel.scroll_to_bottom = true;
                }
            }
            ChatPanelEvent::DeleteSession(id) => {
                self.chat.history().delete(id);
                if self.chat.session_id == Some(id) {
                    self.chat.session_id = None;
                }
                self.chat.panel.sessions = self.chat.history().list();
            }
            ChatPanelEvent::Close => self.chat.panel.open = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_chat::{ModelTurn, ScriptedModel, StopReason, ToolCall};
    use serde_json::json;

    fn pump(app: &mut DuCADApp, ctx: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(120);
        while app.chat.panel.busy && Instant::now() < deadline {
            app.poll_agent_bridge(ctx);
            app.chat_poll(ctx);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!app.chat.panel.busy, "chat tidak selesai");
    }

    #[test]
    fn chat_runs_ops_through_bridge_as_one_undo_step() {
        let mut app = DuCADApp::new_for_test();
        app.chat.history = Some(ChatHistory::in_memory());
        app.chat.loaded = true;
        let ctx = egui::Context::default();
        let model = ScriptedModel::new([
            ModelTurn {
                text: "Membuat blok.".into(),
                calls: vec![ToolCall {
                    id: "c1".into(),
                    name: "run_ops".into(),
                    input: json!({ "ops": [
                        { "op": "primitive", "id": "blok", "shape": { "box": { "size": [10, 20, 30] } } }
                    ] }),
                    input_error: None,
                }],
                raw: None,
                stop: StopReason::ToolUse,
                usage: Usage::default(),
            },
            ModelTurn {
                text: "Selesai.".into(),
                ..Default::default()
            },
        ]);
        app.chat.panel.input = "buat blok".into();
        app.chat_start(&ctx, Box::new(model));
        pump(&mut app, &ctx);

        assert_eq!(app.native_body_refs().len(), 1);
        assert_eq!(app.chat.conv.len(), 4, "{:?}", app.chat.conv);
        let ConvMessage::User { text } = &app.chat.conv[0] else {
            panic!()
        };
        assert!(text.starts_with(CONTEXT_PREFIX), "{text}");
        let tool = app
            .chat
            .panel
            .items
            .iter()
            .find(|i| i.role == ChatRole::Tool)
            .unwrap();
        assert_eq!(tool.ok, Some(true), "{}", tool.text);
        assert!(app
            .chat
            .panel
            .items
            .iter()
            .any(|i| i.role == ChatRole::Assistant && i.text == "Selesai."));
        assert!(app.chat.session_id.is_some(), "riwayat tersimpan");

        // Satu batch agent = satu langkah undo GUI.
        app.model_undo.undo(&mut app.model);
        assert_eq!(app.native_body_refs().len(), 0);

        // Transkrip bisa dibangun ulang dari riwayat tanpa baris konteks.
        let items = items_from_conv(&app.chat.conv);
        assert_eq!(items[0].text, "buat blok");
        assert_eq!(items.iter().filter(|i| i.role == ChatRole::Tool).count(), 1);
    }

    #[test]
    fn external_provider_blocked_when_offline_only() {
        let mut app = DuCADApp::new_for_test();
        app.chat.loaded = true;
        app.chat.settings = ChatSettings::default();
        let ctx = egui::Context::default();
        app.chat.panel.input = "halo".into();
        app.chat_send(&ctx);
        assert!(!app.chat.panel.busy);
        assert_eq!(
            app.chat.panel.items.last().map(|i| i.role),
            Some(ChatRole::Error)
        );
    }

    #[test]
    fn live_tools_exclude_session_and_accept() {
        let names: Vec<String> = live_tools().into_iter().map(|t| t.name).collect();
        assert!(names.contains(&"run_ops".to_string()));
        assert!(names.contains(&"propose_ops".to_string()));
        for banned in ["new_part", "open_part", "close_part", "accept_proposal"] {
            assert!(!names.contains(&banned.to_string()), "{banned}");
        }
        assert!(system_prompt().contains("MODE LIVE"));
    }
}
