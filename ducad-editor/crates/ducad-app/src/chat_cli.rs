//! CLI coding agent sebagai backend Chat AI (P13.5), pola TABULAR.
//!
//! Agent (agy / claude / gemini / kustom) berjalan sebagai proses anak di
//! `~/.ducad/agent-workspace` dan mengendalikan dokumen yang terbuka lewat
//! `ducad-mcp --attach` → soket jembatan agent. Semua tool dieksekusi oleh
//! `agent_bridge.rs` di UI thread, persis seperti chat API dan agent
//! eksternal: satu batch = satu langkah undo, proposal lewat kartu Terima/
//! Tolak, `accept_proposal` tidak tersedia.

#[cfg(not(any(target_os = "ios", target_os = "android")))]
use std::sync::mpsc::Receiver;

#[cfg(not(any(target_os = "ios", target_os = "android")))]
use ducad_chat::ChatEvent;
use ducad_chat::{ChatBackend, ConvMessage, CLI_KINDS};
use ducad_ui::{ChatItem, ChatRole, CliFormProfile, CliMeta};

use crate::app::DuCADApp;
use crate::chat_ui::ChatState;

/// Giliran CLI yang sedang berjalan.
pub struct CliRun {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    pub rx: Receiver<ducad_chat::cli::CliEvent>,
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    pub handle: ducad_chat::cli::CancelHandle,
    pub kind: String,
    /// Ada teks asisten yang sudah dialirkan pada giliran ini.
    pub streamed: String,
}

/// Maksimum karakter riwayat yang disisipkan untuk CLI tanpa resume.
const TRANSCRIPT_BUDGET: usize = 6000;

/// Meta tampilan tiap jenis CLI (urutan = `CLI_KINDS`).
fn cli_meta() -> Vec<CliMeta> {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        use ducad_chat::cli::CliAgentKind;
        CliAgentKind::ALL
            .iter()
            .map(|k| CliMeta {
                name: k.display_name().to_string(),
                presets: k.preset_models().iter().map(|s| s.to_string()).collect(),
                efforts: k.effort_levels().iter().map(|s| s.to_string()).collect(),
                args_hint: match k {
                    CliAgentKind::Custom => "mycli --mcp {mcp_config} -p {prompt}".to_string(),
                    _ => "mis. --max-turns 30".to_string(),
                },
            })
            .collect()
    }
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        Vec::new()
    }
}

fn display_name(kind: &str) -> String {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        ducad_chat::cli::CliAgentKind::from_data_name(kind)
            .display_name()
            .to_string()
    }
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        kind.to_string()
    }
}

impl ChatState {
    /// Isi bagian CLI formulir + daftar backend di kepala panel.
    pub(crate) fn fill_cli_form(&mut self) {
        let f = &mut self.panel.form;
        f.use_cli = self.settings.backend == ChatBackend::Cli;
        f.cli_meta = cli_meta();
        f.cli = CLI_KINDS
            .iter()
            .map(|k| {
                let p = self.settings.cli_profile(k);
                CliFormProfile {
                    enabled: p.enabled,
                    bin: p.bin,
                    model: p.model,
                    effort: p.effort,
                    extra_args: p.extra_args,
                }
            })
            .collect();
        let active = self.settings.active_cli().to_string();
        f.cli_tab = CLI_KINDS.iter().position(|k| *k == active).unwrap_or(0);
        self.refresh_targets();
    }

    /// Daftar backend: API + setiap CLI yang diaktifkan.
    pub(crate) fn refresh_targets(&mut self) {
        let mut targets = vec![format!("API · {}", self.settings.provider.label())];
        let mut idx = 0;
        for k in CLI_KINDS {
            if self.settings.cli_profile(k).enabled && !cli_meta().is_empty() {
                targets.push(format!("CLI · {}", display_name(k)));
                if self.settings.backend == ChatBackend::Cli && self.settings.active_cli() == k {
                    idx = targets.len() - 1;
                }
            }
        }
        self.panel.targets = targets;
        self.panel.target_idx = idx;
        self.panel.provider_label = self.panel.targets.get(idx).cloned().unwrap_or_default();
    }

    /// Salin bagian CLI formulir ke pengaturan.
    pub(crate) fn apply_cli_form(&mut self) {
        let f = self.panel.form.clone();
        for (i, k) in CLI_KINDS.iter().enumerate() {
            let Some(c) = f.cli.get(i) else { continue };
            let p = self.settings.cli_profile_mut(k);
            p.enabled = c.enabled;
            p.bin = c.bin.trim().to_string();
            p.model = c.model.trim().to_string();
            p.effort = c.effort.trim().to_string();
            p.extra_args = c.extra_args.trim().to_string();
        }
        if let Some(k) = CLI_KINDS.get(f.cli_tab) {
            if f.use_cli {
                self.settings.cli_active = k.to_string();
            }
        }
        self.settings.backend = if f.use_cli {
            ChatBackend::Cli
        } else {
            ChatBackend::Api
        };
    }

    /// Riwayat teks singkat untuk CLI yang tidak bisa melanjutkan sesi.
    fn transcript_prefix(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for m in self.conv().iter().rev() {
            let line = match m {
                ConvMessage::User { text } => {
                    format!("Pengguna: {}", crate::chat_ui::strip_context(text))
                }
                ConvMessage::Assistant { text, .. } if !text.trim().is_empty() => {
                    format!("Asisten: {text}")
                }
                _ => continue,
            };
            let used: usize = lines.iter().map(String::len).sum();
            if used + line.len() > TRANSCRIPT_BUDGET {
                break;
            }
            lines.push(line);
        }
        if lines.is_empty() {
            return String::new();
        }
        lines.reverse();
        format!(
            "PERCAKAPAN SEBELUMNYA\n{}\n\nPESAN BARU\n",
            lines.join("\n")
        )
    }
}

impl DuCADApp {
    /// Pilih backend dari kepala panel.
    pub(crate) fn chat_select_target(&mut self, idx: usize) {
        let enabled: Vec<&str> = CLI_KINDS
            .iter()
            .copied()
            .filter(|k| self.chat.settings.cli_profile(k).enabled)
            .collect();
        if idx == 0 {
            self.chat.settings.backend = ChatBackend::Api;
        } else if let Some(k) = enabled.get(idx - 1) {
            self.chat.settings.backend = ChatBackend::Cli;
            self.chat.settings.cli_active = k.to_string();
        }
        if let Err(e) = self
            .chat
            .settings
            .save(&ducad_chat::ChatSettings::default_path())
        {
            log::warn!("gagal menyimpan ai-chat.json: {e:#}");
        }
        self.chat.fill_cli_form();
    }

    fn chat_error(&mut self, msg: String) {
        self.chat.push_item(ChatItem::text(ChatRole::Error, msg));
    }

    /// Kirim isi input ke CLI agent aktif.
    pub(crate) fn chat_send_cli(&mut self, ctx: &egui::Context) {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            let _ = ctx;
            self.chat_error(ducad_i18n::t!("chat-cli-desktop-only"));
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            use ducad_chat::cli::{self, CliAgentProfile, CliRequest};
            let text = self.chat.panel.input.trim().to_string();
            if text.is_empty() || self.chat.is_running() {
                return;
            }
            if !self.chat.settings.allow_external {
                self.chat_error(ducad_i18n::t!("chat-cli-needs-external"));
                return;
            }
            let kind = self.chat.settings.active_cli().to_string();
            let data = self.chat.settings.cli_profile(&kind);
            if !data.enabled {
                self.chat_error(ducad_i18n::t!("chat-cli-none-enabled"));
                return;
            }
            let profile = CliAgentProfile::from(&data);
            let Some(mcp) = cli::ducad_mcp_exe() else {
                self.chat_error(ducad_i18n::t!("chat-cli-no-mcp"));
                return;
            };
            // Agent memanggil tool lewat soket jembatan.
            self.sync_ai_privacy();
            if !self.bridge.enabled {
                if let Err(e) = self.bridge.start(ctx.clone()) {
                    let e = format!("{e:#}");
                    self.chat_error(ducad_i18n::t!("chat-cli-bridge-failed", error = e.as_str()));
                    return;
                }
                self.model_status = Some(ducad_i18n::t!("bridge-on"));
            }
            let socket = crate::agent_bridge::socket_path();
            let mcp_config = match cli::write_mcp_config(&mcp, &socket) {
                Ok(p) => Some(p),
                Err(e) => {
                    self.chat_error(e);
                    return;
                }
            };
            let resume = profile.kind.supports_resume();
            let session_id = resume
                .then(|| self.chat.cli_sessions.get(&kind).cloned())
                .flatten();
            let history = if resume {
                String::new()
            } else {
                self.chat.transcript_prefix()
            };
            let context = self.chat_context();
            let req = CliRequest {
                system_prompt: crate::chat_ui::system_prompt(),
                user_prompt: format!("{history}{context}\n{text}"),
                session_id,
                cwd: cli::agent_workspace_dir(),
                mcp_config,
            };
            match cli::spawn_stream(&profile, req) {
                Ok((rx, handle)) => {
                    self.chat.panel.input.clear();
                    self.chat
                        .push_item(ChatItem::text(ChatRole::User, text.clone()));
                    self.chat.push_conv(ConvMessage::User {
                        text: format!("{context}\n{text}"),
                    });
                    self.chat.begin_turn();
                    self.chat.cli_run = Some(CliRun {
                        rx,
                        handle,
                        kind,
                        streamed: String::new(),
                    });
                    self.chat.panel.busy = true;
                    ctx.request_repaint();
                }
                Err(e) => self.chat_error(e),
            }
        }
    }

    /// Hentikan proses CLI yang berjalan.
    pub(crate) fn chat_stop_cli(&mut self) {
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        if let Some(run) = &self.chat.cli_run {
            run.handle.cancel();
        }
    }

    /// Terima kejadian CLI (tiap frame).
    pub(crate) fn chat_poll_cli(&mut self, ctx: &egui::Context) {
        self.poll_cli_status();
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            use ducad_chat::cli::CliEvent;
            let Some(mut run) = self.chat.cli_run.take() else {
                return;
            };
            let mut done: Option<(String, Option<String>, bool)> = None;
            loop {
                match run.rx.try_recv() {
                    Ok(CliEvent::Session(id)) => {
                        self.chat.cli_sessions.insert(run.kind.clone(), id);
                    }
                    Ok(CliEvent::Chat(e)) => {
                        let terminal = matches!(e, ChatEvent::Error { .. } | ChatEvent::Cancelled);
                        if let ChatEvent::TextDelta { text } = &e {
                            run.streamed.push_str(text);
                        }
                        self.chat.apply_chat_event(ctx, e);
                        if terminal {
                            done = Some((String::new(), None, false));
                            break;
                        }
                    }
                    Ok(CliEvent::Finished { text, usage }) => {
                        done = Some((text, usage, true));
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        done = Some((String::new(), None, false));
                        break;
                    }
                }
            }
            match done {
                None => {
                    self.chat.cli_run = Some(run);
                    ctx.request_repaint_after(std::time::Duration::from_millis(40));
                }
                Some((text, usage, ok)) => {
                    let answer = if run.streamed.trim().is_empty() {
                        text
                    } else {
                        run.streamed.clone()
                    };
                    if ok && run.streamed.trim().is_empty() && !answer.trim().is_empty() {
                        self.chat
                            .push_item(ChatItem::text(ChatRole::Assistant, answer.clone()));
                    }
                    if let Some(u) = usage {
                        self.chat.panel.usage_label = u;
                    }
                    if !answer.trim().is_empty() {
                        self.chat.push_conv(ConvMessage::Assistant {
                            text: answer,
                            calls: Vec::new(),
                            raw: None,
                        });
                    }
                    self.chat.panel.busy = false;
                    self.chat.end_turn();
                }
            }
        }
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let _ = ctx;
    }

    /// Hasil tugas latar Detect/Daftarkan/Uji.
    fn poll_cli_status(&mut self) {
        let Some(rx) = self.chat.cli_bg.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(msg) => {
                self.chat.panel.form.cli_status = msg;
                self.chat.panel.form.cli_busy = false;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => self.chat.cli_bg = Some(rx),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.chat.panel.form.cli_busy = false
            }
        }
    }

    fn form_profile(&self, i: usize) -> Option<ducad_chat::CliProfileData> {
        let kind = CLI_KINDS.get(i)?;
        let c = self.chat.panel.form.cli.get(i)?;
        Some(ducad_chat::CliProfileData {
            kind: kind.to_string(),
            enabled: c.enabled,
            bin: c.bin.trim().to_string(),
            model: c.model.trim().to_string(),
            effort: c.effort.trim().to_string(),
            extra_args: c.extra_args.trim().to_string(),
        })
    }

    /// Tombol Deteksi: cari binary dan isi path-nya.
    pub(crate) fn chat_cli_detect(&mut self, i: usize) {
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            let Some(data) = self.form_profile(i) else {
                return;
            };
            let p = ducad_chat::cli::CliAgentProfile::from(&data);
            let name = p.effective_bin();
            match ducad_chat::cli::resolve_binary(&name) {
                Some(path) => {
                    let s = path.to_string_lossy().to_string();
                    if let Some(c) = self.chat.panel.form.cli.get_mut(i) {
                        c.bin = s.clone();
                    }
                    self.chat.panel.form.cli_status =
                        ducad_i18n::t!("chat-cli-detected", path = s.as_str());
                }
                None => {
                    self.chat.panel.form.cli_status =
                        ducad_i18n::t!("chat-cli-not-found", name = name.as_str());
                }
            }
        }
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            let _ = i;
            self.chat.panel.form.cli_status = ducad_i18n::t!("chat-cli-desktop-only");
        }
    }

    /// Tombol Daftarkan MCP / Uji koneksi (di thread latar: bisa detik-an).
    pub(crate) fn chat_cli_background(&mut self, i: usize, register: bool) {
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            use ducad_chat::cli;
            let Some(data) = self.form_profile(i) else {
                return;
            };
            let p = cli::CliAgentProfile::from(&data);
            let mcp = cli::ducad_mcp_exe();
            let socket = crate::agent_bridge::socket_path();
            let (tx, rx) = std::sync::mpsc::channel();
            self.chat.cli_bg = Some(rx);
            self.chat.panel.form.cli_busy = true;
            self.chat.panel.form.cli_status = ducad_i18n::t!("chat-cli-working");
            std::thread::spawn(move || {
                let msg = if register {
                    match mcp {
                        None => ducad_i18n::t!("chat-cli-no-mcp"),
                        Some(m) => match cli::register_mcp(&p, &m, &socket) {
                            Ok(out) => format!("✔ {out}"),
                            Err(e) => format!("✖ {e}"),
                        },
                    }
                } else {
                    let mut msg = match cli::test_connection(&p) {
                        Ok(s) => format!("✔ {s}"),
                        Err(e) => format!("✖ {e}"),
                    };
                    if p.kind.needs_global_mcp() {
                        match cli::check_mcp_registered(&p) {
                            Ok(true) => msg.push_str("\n✔ MCP ducad terdaftar"),
                            Ok(false) => {
                                let n = p.kind.display_name();
                                msg.push('\n');
                                msg.push_str(&ducad_i18n::t!("chat-cli-not-registered", name = n));
                            }
                            Err(e) => msg.push_str(&format!("\n? {e}")),
                        }
                    }
                    msg
                };
                let _ = tx.send(msg);
            });
        }
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            let _ = (i, register);
            self.chat.panel.form.cli_status = ducad_i18n::t!("chat-cli-desktop-only");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        app.chat.loaded_for_test();
        app
    }

    #[test]
    fn targets_follow_enabled_profiles() {
        let mut app = app();
        app.chat.settings.cli_profile_mut("claude_code").enabled = true;
        app.chat.fill_cli_form();
        assert_eq!(
            app.chat.panel.targets.len(),
            2,
            "{:?}",
            app.chat.panel.targets
        );
        assert_eq!(app.chat.panel.target_idx, 0);
        app.chat.settings.backend = ChatBackend::Cli;
        app.chat.settings.cli_active = "claude_code".into();
        app.chat.refresh_targets();
        assert_eq!(app.chat.panel.target_idx, 1);
        assert!(app.chat.panel.provider_label.contains("Claude Code"));
        // Formulir → pengaturan.
        app.chat.panel.form.cli[2].enabled = true;
        app.chat.panel.form.cli[2].model = " gemini-2.5-pro ".into();
        app.chat.panel.form.use_cli = true;
        app.chat.panel.form.cli_tab = 2;
        app.chat.apply_cli_form();
        assert_eq!(app.chat.settings.cli_active, "gemini_cli");
        assert_eq!(
            app.chat.settings.cli_profile("gemini_cli").model,
            "gemini-2.5-pro"
        );
    }

    #[test]
    fn cli_needs_external_permission() {
        let mut app = app();
        app.chat.settings.backend = ChatBackend::Cli;
        app.chat.panel.input = "halo".into();
        let ctx = egui::Context::default();
        app.chat_send_cli(&ctx);
        assert!(!app.chat.panel.busy);
        assert_eq!(
            app.chat.panel.items.last().map(|i| i.role),
            Some(ChatRole::Error)
        );
    }

    #[cfg(unix)]
    #[test]
    fn cli_stream_fills_transcript_and_history() {
        use ducad_chat::cli::{spawn_stream, CliAgentKind, CliAgentProfile, CliRequest};
        let dir = std::env::temp_dir().join(format!("ducad-app-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lines = [
            json!({"type":"system","subtype":"init","session_id":"sess-7"}),
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"Membuat blok."},{"type":"tool_use","id":"t1","name":"mcp__ducad__run_ops","input":{"ops":[]}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"{\"committed\":true}"}]}}),
            json!({"type":"result","is_error":false,"result":"Membuat blok.","usage":{"input_tokens":3,"output_tokens":4}}),
        ];
        let body: String = lines.iter().map(|l| format!("echo '{l}'\n")).collect();
        let script = dir.join("fake-claude.sh");
        std::fs::write(&script, format!("#!/bin/sh\n{body}")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut p = CliAgentProfile::new(CliAgentKind::ClaudeCode);
        p.bin = script.to_string_lossy().to_string();
        let req = CliRequest {
            system_prompt: "S".into(),
            user_prompt: "U".into(),
            session_id: None,
            cwd: dir.clone(),
            mcp_config: None,
        };
        let (rx, handle) = spawn_stream(&p, req).unwrap();

        let mut app = app();
        app.chat.push_conv(ConvMessage::User {
            text: "buat blok".into(),
        });
        app.chat.cli_run = Some(CliRun {
            rx,
            handle,
            kind: "claude_code".into(),
            streamed: String::new(),
        });
        app.chat.panel.busy = true;
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while app.chat.panel.busy && std::time::Instant::now() < deadline {
            app.chat_poll_cli(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!app.chat.panel.busy);
        assert_eq!(
            app.chat.cli_sessions.get("claude_code").map(String::as_str),
            Some("sess-7")
        );
        let tool = app
            .chat
            .panel
            .items
            .iter()
            .find(|i| i.role == ChatRole::Tool)
            .expect("kartu tool");
        assert_eq!(tool.tool, "run_ops");
        assert_eq!(tool.ok, Some(true));
        assert!(app
            .chat
            .panel
            .items
            .iter()
            .any(|i| i.role == ChatRole::Assistant && i.text == "Membuat blok."));
        assert!(app.chat.panel.usage_label.contains("3 masuk"));
        assert!(
            matches!(app.chat.conv().last(), Some(ConvMessage::Assistant { text, .. }) if text == "Membuat blok.")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn force_propose_turns_run_ops_into_proposal() {
        let mut app = DuCADApp::new_for_test();
        app.bridge.force_propose = true;
        let (tx, _rx) = std::sync::mpsc::channel();
        let ops = json!({ "ops": [{ "op": "primitive", "id": "b", "shape": { "box": { "size": [1, 1, 1] } } }] });
        let dry = app.agent_call_for_test(
            "run_ops",
            json!({ "ops": ops["ops"], "dry_run": true }),
            1,
            &tx,
        );
        assert!(dry.is_some(), "dry run tetap langsung dijawab");
        let out = app.agent_call_for_test("run_ops", ops, 2, &tx);
        assert!(out.is_none(), "balasan menunggu keputusan pengguna");
        assert!(app.native_body_refs().is_empty(), "model belum berubah");
    }
}
