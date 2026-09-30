//! Panel chat AI (P13.3): percakapan dengan agent yang mengendalikan
//! DUCAD lewat tool yang sama dengan server MCP. Widget murni — state dan
//! logika ada di `ducad-app/src/chat_ui.rs`.

use crate::theme::{
    glass_frame, ACCENT_BLUE, ACCENT_GREEN, BG_CARD_DARK, BG_HOVER_DARK, BORDER_SUBTLE,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{Color32, RichText};
use egui_icons::icons::ICON_CLOSE;

/// Warna error (sama dengan ringkasan checks gagal di top bar).
const ERROR_RED: Color32 = Color32::from_rgb(255, 69, 58);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    Tool,
    Notice,
    Error,
}

/// Satu baris transkrip.
#[derive(Clone)]
pub struct ChatItem {
    pub role: ChatRole,
    pub text: String,
    /// Kartu tool: nama tool.
    pub tool: String,
    /// Kartu tool: `None` = berjalan, `Some(true)` = sukses.
    pub ok: Option<bool>,
    /// Kartu tool: argumen ringkas (JSON satu baris).
    pub detail: String,
    /// Gambar hasil tool (mis. render_view).
    pub image: Option<egui::TextureHandle>,
}

impl ChatItem {
    pub fn text(role: ChatRole, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            tool: String::new(),
            ok: None,
            detail: String::new(),
            image: None,
        }
    }
}

/// Isian formulir pengaturan provider.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatProviderForm {
    /// 0 = Anthropic, 1 = OpenAI, 2 = Ollama/lokal, 3 = OpenAI-compatible lain.
    pub preset: usize,
    pub base_url: String,
    pub model: String,
    /// Kunci baru (kosong = biarkan yang tersimpan).
    pub api_key_input: String,
    pub key_saved: bool,
    pub confirm_writes: bool,
    pub allow_external: bool,
    /// `true` = backend CLI agent.
    pub use_cli: bool,
    /// Tab CLI yang dibuka di pengaturan.
    pub cli_tab: usize,
    pub cli: Vec<CliFormProfile>,
    pub cli_meta: Vec<CliMeta>,
    /// Hasil Detect/Daftarkan MCP/Uji koneksi.
    pub cli_status: String,
    pub cli_busy: bool,
}

/// Isian satu profil CLI agent di formulir.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CliFormProfile {
    pub enabled: bool,
    pub bin: String,
    pub model: String,
    pub effort: String,
    pub extra_args: String,
}

/// Info statis satu jenis CLI agent (diisi aplikasi).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CliMeta {
    pub name: String,
    pub presets: Vec<String>,
    pub efforts: Vec<String>,
    /// Placeholder argumen tambahan.
    pub args_hint: String,
}

pub const PRESETS: [&str; 4] = [
    "Anthropic (Claude)",
    "OpenAI",
    "Ollama / LM Studio (lokal)",
    "OpenAI-compatible",
];

#[derive(Default)]
pub struct ChatPanelState {
    pub open: bool,
    pub items: Vec<ChatItem>,
    pub input: String,
    pub busy: bool,
    /// Label provider/model aktif.
    pub provider_label: String,
    /// Ringkasan token terakhir.
    pub usage_label: String,
    pub settings_open: bool,
    pub form: ChatProviderForm,
    pub history_open: bool,
    /// `(id, judul, waktu)` sesi tersimpan.
    pub sessions: Vec<(i64, String, String)>,
    /// Konfirmasi dua klik "Chat baru" (waktu egui kedaluwarsa).
    pub clear_armed_until: Option<f64>,
    /// Gulir ke bawah pada frame berikutnya.
    pub scroll_to_bottom: bool,
    /// Pilihan backend di kepala panel (label) dan indeks terpilih.
    pub targets: Vec<String>,
    pub target_idx: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatPanelEvent {
    Send,
    Stop,
    NewChat,
    SaveSettings,
    PresetChanged(usize),
    OpenHistory,
    LoadSession(i64),
    DeleteSession(i64),
    Close,
    /// Pilih backend dari kepala panel (indeks `targets`).
    TargetChanged(usize),
    /// Cari binary CLI tab ini.
    CliDetect(usize),
    /// Daftarkan server MCP DUCAD di CLI tab ini.
    CliRegisterMcp(usize),
    /// Uji koneksi CLI tab ini.
    CliTest(usize),
}

pub struct ChatPanel;

/// Render teks sederhana ala markdown: blok kode ```, judul #, butir -/*.
fn markdown_lite(ui: &mut egui::Ui, text: &str, color: Color32) {
    let mut in_code = false;
    let mut code = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            if in_code {
                ui.add(
                    egui::Label::new(
                        RichText::new(code.trim_end())
                            .monospace()
                            .size(11.0)
                            .color(color),
                    )
                    .selectable(true),
                );
                code.clear();
            }
            in_code = !in_code;
            continue;
        }
        if in_code {
            code.push_str(line);
            code.push('\n');
            continue;
        }
        let trimmed = line.trim_start();
        let rich = if let Some(h) = trimmed
            .strip_prefix("#")
            .map(|s| s.trim_start_matches('#').trim())
        {
            RichText::new(h).strong().color(color)
        } else if let Some(b) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            RichText::new(format!("• {}", b.replace("**", ""))).color(color)
        } else {
            RichText::new(line.replace("**", "")).color(color)
        };
        ui.add(egui::Label::new(rich).wrap().selectable(true));
    }
    if in_code && !code.is_empty() {
        ui.add(
            egui::Label::new(
                RichText::new(code.trim_end())
                    .monospace()
                    .size(11.0)
                    .color(color),
            )
            .selectable(true),
        );
    }
}

fn item_ui(ui: &mut egui::Ui, item: &ChatItem) {
    match item.role {
        ChatRole::User => {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                egui::Frame::new()
                    .fill(ACCENT_BLUE.gamma_multiply(0.35))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width() * 0.85);
                        ui.add(
                            egui::Label::new(RichText::new(&item.text).color(TEXT_PRIMARY))
                                .wrap()
                                .selectable(true),
                        );
                    });
            });
        }
        ChatRole::Assistant => markdown_lite(ui, &item.text, TEXT_PRIMARY),
        // Kartu tool digambar per kelompok berurutan di `tool_group_ui`.
        ChatRole::Tool => tool_group_ui(ui, std::slice::from_ref(item), usize::MAX),
        ChatRole::Notice => {
            ui.add(
                egui::Label::new(
                    RichText::new(&item.text)
                        .italics()
                        .size(11.0)
                        .color(TEXT_SECONDARY),
                )
                .wrap(),
            );
        }
        ChatRole::Error => {
            ui.add(
                egui::Label::new(RichText::new(&item.text).color(ERROR_RED))
                    .wrap()
                    .selectable(true),
            );
        }
    }
}

/// Latar blok kode di detail tool.
const CODE_BG: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 90);

/// Nama tool yang enak dibaca: `mcp__ducad__run_ops` → "Run ops".
fn tool_title(name: &str) -> String {
    let base = name.rsplit("__").next().unwrap_or(name);
    let spaced = base.replace(['_', '-'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => name.to_string(),
    }
}

/// Persingkat path panjang menjadi dua komponen terakhir.
fn short_path(s: &str) -> String {
    let parts: Vec<&str> = s.trim_end_matches('/').rsplit('/').take(3).collect();
    if parts.len() < 3 {
        return s.to_string();
    }
    format!("…/{}/{}", parts[1], parts[0])
}

/// Ringkasan satu baris dari argumen tool (perintah, path, jumlah op, …).
fn tool_summary(detail: &str) -> String {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(detail)
    else {
        return detail.lines().next().unwrap_or("").to_string();
    };
    const COMMAND_KEYS: [&str; 3] = ["command", "CommandLine", "cmd"];
    const PATH_KEYS: [&str; 6] = [
        "file_path",
        "path",
        "AbsolutePath",
        "TargetFile",
        "DirectoryPath",
        "SearchPath",
    ];
    const OTHER_KEYS: [&str; 8] = [
        "url", "Url", "query", "Query", "pattern", "Pattern", "name", "part",
    ];
    let first_line = |v: &serde_json::Value| {
        v.as_str()
            .map(|s| s.lines().next().unwrap_or("").trim().to_string())
    };
    for k in COMMAND_KEYS {
        if let Some(s) = map.get(k).and_then(first_line) {
            return format!("$ {s}");
        }
    }
    for k in PATH_KEYS {
        if let Some(s) = map.get(k).and_then(first_line) {
            return short_path(&s);
        }
    }
    for k in OTHER_KEYS {
        if let Some(s) = map.get(k).and_then(first_line) {
            return s;
        }
    }
    if let Some(ops) = map.get("ops").and_then(|v| v.as_array()) {
        return format!("{} op", ops.len());
    }
    map.values().find_map(first_line).unwrap_or_default()
}

/// JSON argumen yang dirapikan; teks mentah bila tidak valid (mis. terpotong).
fn pretty_json(detail: &str) -> String {
    serde_json::from_str::<serde_json::Value>(detail)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| detail.to_string())
}

/// Blok kode monospace yang bisa diseleksi, digulir bila panjang.
fn code_block(ui: &mut egui::Ui, id: egui::Id, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(CODE_BG)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt(id)
                .max_height(180.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(text).monospace().size(10.5).color(color))
                            .wrap()
                            .selectable(true),
                    );
                });
        });
}

/// Judul kecil bagian detail ("INPUT", "HASIL").
fn section_label(ui: &mut egui::Ui, text: String) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(9.5)
            .strong()
            .color(TEXT_MUTED),
    );
}

/// Satu kelompok panggilan tool berurutan dalam satu kartu.
/// `first_idx` = indeks item pertama di transkrip (untuk id widget unik).
fn tool_group_ui(ui: &mut egui::Ui, items: &[ChatItem], first_idx: usize) {
    egui::Frame::new()
        .fill(BG_CARD_DARK)
        .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(4, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    let y = ui.cursor().top();
                    let x = ui.max_rect().x_range();
                    ui.painter().hline(
                        (x.min + 6.0)..=(x.max - 6.0),
                        y,
                        egui::Stroke::new(1.0, BORDER_SUBTLE),
                    );
                }
                tool_row_ui(ui, item, first_idx.saturating_add(i));
            }
        });
}

/// Satu baris tool: status · nama · ringkasan argumen; klik untuk detail.
fn tool_row_ui(ui: &mut egui::Ui, item: &ChatItem, idx: usize) {
    let id = ui.make_persistent_id(("chat-tool", idx));
    let has_body = !item.detail.is_empty() || !item.text.is_empty() || item.image.is_some();
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let failed = item.ok == Some(false);

    // Latar hover disisipkan di bawah isi baris setelah ukurannya diketahui.
    let bg = ui.painter().add(egui::Shape::Noop);
    let header = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(6, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                match item.ok {
                    None => {
                        ui.add(egui::Spinner::new().size(11.0));
                    }
                    Some(true) => {
                        ui.label(RichText::new("✓").size(11.0).color(ACCENT_GREEN));
                    }
                    Some(false) => {
                        ui.label(RichText::new(ICON_CLOSE.codepoint).size(11.0).color(ERROR_RED));
                    }
                }
                ui.label(
                    RichText::new(tool_title(&item.tool))
                        .size(12.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                let summary = if item.ok.is_none() && item.detail.is_empty() {
                    t!("chat-tool-running")
                } else {
                    tool_summary(&item.detail)
                };
                let chevron_w = if has_body { 14.0 } else { 0.0 };
                ui.scope(|ui| {
                    ui.set_max_width((ui.available_width() - chevron_w).max(0.0));
                    ui.add(
                        egui::Label::new(
                            RichText::new(summary)
                                .monospace()
                                .size(10.5)
                                .color(TEXT_SECONDARY),
                        )
                        .truncate()
                        .selectable(false),
                    );
                });
                if has_body {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(if open { "⏷" } else { "⏵" })
                                .size(10.0)
                                .color(TEXT_MUTED),
                        );
                    });
                }
            });
        })
        .response;
    let resp = ui.interact(header.rect, id.with("hdr"), egui::Sense::click());
    if has_body && resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    if has_body {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    if ui.rect_contains_pointer(header.rect) && has_body {
        ui.painter().set(
            bg,
            egui::Shape::rect_filled(header.rect, 6.0, BG_HOVER_DARK),
        );
    }

    // Galat tetap terlihat meski baris tertutup.
    if failed && !open && !item.text.is_empty() {
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 23,
                right: 6,
                top: 0,
                bottom: 5,
            })
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(RichText::new(&item.text).size(10.5).color(ERROR_RED))
                        .wrap()
                        .selectable(true),
                );
            });
    }

    if open && has_body {
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 23,
                right: 6,
                top: 0,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                if !item.detail.is_empty() {
                    section_label(ui, t!("chat-tool-input"));
                    code_block(ui, id.with("in"), &pretty_json(&item.detail), TEXT_PRIMARY);
                }
                if !item.text.is_empty() {
                    ui.add_space(2.0);
                    section_label(ui, t!("chat-tool-output"));
                    let color = if failed { ERROR_RED } else { TEXT_SECONDARY };
                    code_block(ui, id.with("out"), &item.text, color);
                }
                if let Some(tex) = &item.image {
                    ui.add_space(2.0);
                    let w = ui.available_width().min(320.0);
                    let size = tex.size_vec2();
                    let h = if size.x > 0.0 { w * size.y / size.x } else { w };
                    ui.add(egui::Image::new((tex.id(), egui::vec2(w, h))).corner_radius(6.0));
                }
            });
    }
    // Gambar hasil (render_view) tetap tampil sebagai pratinjau saat tertutup.
    if !open {
        if let Some(tex) = &item.image {
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 23,
                    right: 6,
                    top: 0,
                    bottom: 6,
                })
                .show(ui, |ui| {
                    let w = ui.available_width().min(320.0);
                    let size = tex.size_vec2();
                    let h = if size.x > 0.0 { w * size.y / size.x } else { w };
                    ui.add(egui::Image::new((tex.id(), egui::vec2(w, h))).corner_radius(6.0));
                });
        }
    }
}

fn cli_settings_ui(
    ui: &mut egui::Ui,
    form: &mut ChatProviderForm,
    event: &mut Option<ChatPanelEvent>,
) {
    ui.label(
        RichText::new(t!("chat-cli-intro"))
            .size(10.0)
            .color(TEXT_SECONDARY),
    );
    ui.horizontal_wrapped(|ui| {
        for (i, meta) in form.cli_meta.iter().enumerate() {
            let on = form.cli.get(i).is_some_and(|c| c.enabled);
            let label = if on {
                format!("{} ●", meta.name)
            } else {
                meta.name.clone()
            };
            ui.selectable_value(&mut form.cli_tab, i, label);
        }
    });
    let i = form.cli_tab.min(form.cli.len().saturating_sub(1));
    let (Some(meta), Some(c)) = (form.cli_meta.get(i).cloned(), form.cli.get_mut(i)) else {
        return;
    };
    ui.checkbox(&mut c.enabled, t!("chat-cli-enable"));
    egui::Grid::new("chat-cli-grid")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label(t!("chat-cli-command"));
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut c.bin)
                        .hint_text(t!("chat-cli-command-hint"))
                        .desired_width(170.0),
                );
                if ui.button(t!("chat-cli-detect")).clicked() {
                    *event = Some(ChatPanelEvent::CliDetect(i));
                }
            });
            ui.end_row();
            ui.label(t!("chat-model"));
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut c.model)
                        .hint_text(t!("chat-cli-model-default"))
                        .desired_width(170.0),
                );
                if ui.button(t!("chat-cli-default")).clicked() {
                    c.model.clear();
                }
            });
            ui.end_row();
        });
    if !meta.presets.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(t!("chat-cli-quick-pick"))
                    .size(10.0)
                    .color(TEXT_SECONDARY),
            );
            for m in &meta.presets {
                if ui
                    .selectable_label(c.model == *m, RichText::new(m).size(11.0))
                    .clicked()
                {
                    c.model = m.clone();
                }
            }
        });
    }
    if !meta.efforts.is_empty() {
        ui.horizontal(|ui| {
            ui.label(t!("chat-cli-effort"));
            let shown = if c.effort.is_empty() {
                t!("chat-cli-effort-default")
            } else {
                c.effort.clone()
            };
            egui::ComboBox::from_id_salt(("chat-cli-effort", i))
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut c.effort,
                        String::new(),
                        t!("chat-cli-effort-default"),
                    );
                    for e in &meta.efforts {
                        ui.selectable_value(&mut c.effort, e.clone(), e.as_str());
                    }
                });
        });
    }
    ui.horizontal(|ui| {
        ui.label(t!("chat-cli-extra-args"));
        ui.add(
            egui::TextEdit::singleline(&mut c.extra_args)
                .hint_text(meta.args_hint.as_str())
                .desired_width(220.0),
        );
    });
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!form.cli_busy, |ui| {
            if ui
                .button(t!("chat-cli-register"))
                .on_hover_text(t!("chat-cli-register-hint"))
                .clicked()
            {
                *event = Some(ChatPanelEvent::CliRegisterMcp(i));
            }
            if ui.button(t!("chat-cli-test")).clicked() {
                *event = Some(ChatPanelEvent::CliTest(i));
            }
        });
        if form.cli_busy {
            ui.spinner();
        }
    });
    if !form.cli_status.is_empty() {
        ui.add(
            egui::Label::new(
                RichText::new(&form.cli_status)
                    .size(10.0)
                    .color(TEXT_SECONDARY),
            )
            .wrap()
            .selectable(true),
        );
    }
}

fn settings_ui(ui: &mut egui::Ui, form: &mut ChatProviderForm, event: &mut Option<ChatPanelEvent>) {
    ui.horizontal(|ui| {
        ui.label(t!("chat-backend"));
        ui.selectable_value(&mut form.use_cli, false, t!("chat-backend-api"));
        ui.selectable_value(&mut form.use_cli, true, t!("chat-backend-cli"));
    });
    if form.use_cli {
        cli_settings_ui(ui, form, event);
    } else {
        api_settings_ui(ui, form, event);
    }
    ui.separator();
    ui.checkbox(&mut form.confirm_writes, t!("chat-confirm-writes"));
    ui.checkbox(&mut form.allow_external, t!("chat-allow-external"));
    ui.label(
        RichText::new(t!("chat-privacy-note"))
            .size(10.0)
            .color(TEXT_SECONDARY),
    );
    if ui.button(t!("chat-save")).clicked() {
        *event = Some(ChatPanelEvent::SaveSettings);
    }
}

fn api_settings_ui(
    ui: &mut egui::Ui,
    form: &mut ChatProviderForm,
    event: &mut Option<ChatPanelEvent>,
) {
    egui::Grid::new("chat-settings")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label(t!("chat-provider"));
            let before = form.preset;
            egui::ComboBox::from_id_salt("chat-preset")
                .selected_text(PRESETS.get(form.preset).copied().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (i, p) in PRESETS.iter().enumerate() {
                        ui.selectable_value(&mut form.preset, i, *p);
                    }
                });
            if form.preset != before {
                *event = Some(ChatPanelEvent::PresetChanged(form.preset));
            }
            ui.end_row();
            ui.label(t!("chat-base-url"));
            ui.add(egui::TextEdit::singleline(&mut form.base_url).desired_width(220.0));
            ui.end_row();
            ui.label(t!("chat-model"));
            ui.add(egui::TextEdit::singleline(&mut form.model).desired_width(220.0));
            ui.end_row();
            ui.label(t!("chat-api-key"));
            let hint = if form.key_saved {
                t!("chat-key-saved")
            } else {
                t!("chat-key-empty")
            };
            ui.add(
                egui::TextEdit::singleline(&mut form.api_key_input)
                    .password(true)
                    .hint_text(hint)
                    .desired_width(220.0),
            );
            ui.end_row();
        });
}

impl ChatPanel {
    /// Render sidebar chat di sisi kanan `ui` (dipanggil sebelum
    /// `CentralPanel` supaya viewport menyempit, bukan tertimpa).
    pub fn show(ui: &mut egui::Ui, state: &mut ChatPanelState) -> Option<ChatPanelEvent> {
        let mut event = None;
        let now = ui.input(|i| i.time);
        let avail_w = ui.available_width();
        let max_w = (avail_w * 0.6).max(SIDEBAR_MIN_W);
        let default_w = SIDEBAR_DEFAULT_W.min(avail_w * 0.35).max(SIDEBAR_MIN_W);
        egui::Panel::right(egui::Id::new("ducad-chat-sidebar"))
            .resizable(true)
            .drag_to_open(false)
            .default_size(default_w)
            .size_range(SIDEBAR_MIN_W..=max_w)
            .frame(sidebar_frame())
            .show_collapsible(ui, &mut state.open, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("chat-title"))
                            .strong()
                            .size(13.0)
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(ICON_CLOSE.codepoint)
                            .on_hover_text(t!("chat-close"))
                            .clicked()
                        {
                            event = Some(ChatPanelEvent::Close);
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if state.targets.len() > 1 && !state.busy {
                        let before = state.target_idx;
                        let shown = state
                            .targets
                            .get(state.target_idx)
                            .cloned()
                            .unwrap_or_default();
                        egui::ComboBox::from_id_salt("chat-target")
                            .width(160.0)
                            .selected_text(RichText::new(shown).size(11.0))
                            .show_ui(ui, |ui| {
                                for (i, t) in state.targets.iter().enumerate() {
                                    ui.selectable_value(&mut state.target_idx, i, t.as_str());
                                }
                            });
                        if state.target_idx != before {
                            event = Some(ChatPanelEvent::TargetChanged(state.target_idx));
                        }
                    } else {
                        ui.label(
                            RichText::new(&state.provider_label)
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("⚙")
                            .on_hover_text(t!("chat-settings"))
                            .clicked()
                        {
                            state.settings_open = !state.settings_open;
                        }
                        if ui
                            .small_button("🕘")
                            .on_hover_text(t!("chat-history"))
                            .clicked()
                        {
                            state.history_open = !state.history_open;
                            if state.history_open {
                                event = Some(ChatPanelEvent::OpenHistory);
                            }
                        }
                        let armed = state.clear_armed_until.is_some_and(|t| t > now);
                        let label = if armed {
                            t!("chat-new-confirm")
                        } else {
                            t!("chat-new")
                        };
                        if ui
                            .add_enabled(
                                !state.busy,
                                egui::Button::new(RichText::new(label).size(11.0)),
                            )
                            .clicked()
                        {
                            if armed {
                                state.clear_armed_until = None;
                                event = Some(ChatPanelEvent::NewChat);
                            } else {
                                state.clear_armed_until = Some(now + 3.0);
                            }
                        }
                    });
                });
                if state.settings_open {
                    ui.separator();
                    settings_ui(ui, &mut state.form, &mut event);
                }
                if state.history_open {
                    ui.separator();
                    if state.sessions.is_empty() {
                        ui.label(
                            RichText::new(t!("chat-history-empty"))
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                        );
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("chat-sessions")
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for (id, title, when) in &state.sessions {
                                ui.horizontal(|ui| {
                                    if ui.link(title).on_hover_text(when).clicked() {
                                        event = Some(ChatPanelEvent::LoadSession(*id));
                                    }
                                    if ui.small_button("🗑").clicked() {
                                        event = Some(ChatPanelEvent::DeleteSession(*id));
                                    }
                                });
                            }
                        });
                }
                ui.separator();
                let input_h = if state.busy { 108.0 } else { 84.0 };
                let avail = (ui.available_height() - input_h).max(120.0);
                // JANGAN memakai `vertical_scroll_offset(f32::MAX)`: pada frame
                // itu seluruh isi digambar di luar area pandang, dan saat agent
                // mengalirkan kejadian tiap frame transkrip tampak kosong terus.
                let follow = std::mem::take(&mut state.scroll_to_bottom);
                let scroll = egui::ScrollArea::vertical()
                    .id_salt("chat-transcript")
                    .max_height(avail)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true);
                scroll.show(ui, |ui| {
                    if state.items.is_empty() {
                        ui.label(RichText::new(t!("chat-empty")).color(TEXT_SECONDARY));
                    }
                    // Panggilan tool berurutan disatukan dalam satu kartu.
                    let mut i = 0;
                    while i < state.items.len() {
                        if state.items[i].role == ChatRole::Tool {
                            let start = i;
                            while i < state.items.len() && state.items[i].role == ChatRole::Tool {
                                i += 1;
                            }
                            tool_group_ui(ui, &state.items[start..i], start);
                        } else {
                            item_ui(ui, &state.items[i]);
                            i += 1;
                        }
                        ui.add_space(6.0);
                    }
                    if follow {
                        ui.scroll_to_cursor_animation(
                            Some(egui::Align::BOTTOM),
                            egui::style::ScrollAnimation::none(),
                        );
                    }
                });
                // Status tetap di luar area gulir: selalu terlihat selama
                // agent bekerja, berapa pun panjang transkripnya.
                if state.busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new(t!("chat-working"))
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                        );
                    });
                }
                ui.separator();
                let resp = ui.add(
                    egui::TextEdit::multiline(&mut state.input)
                        .hint_text(t!("chat-hint"))
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                let enter = resp.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                ui.horizontal(|ui| {
                    if state.busy {
                        if ui.button(t!("chat-stop")).clicked() {
                            event = Some(ChatPanelEvent::Stop);
                        }
                    } else {
                        let can = !state.input.trim().is_empty();
                        let btn =
                            egui::Button::new(RichText::new(t!("chat-send")).color(Color32::WHITE))
                                .fill(ACCENT_BLUE);
                        if ui.add_enabled(can, btn).clicked() || (enter && can) {
                            if enter {
                                // Enter menyisipkan baris baru; buang sebelum dikirim.
                                let trimmed = state.input.trim_end_matches('\n').to_string();
                                state.input = trimmed;
                            }
                            event = Some(ChatPanelEvent::Send);
                        }
                    }
                    if !state.usage_label.is_empty() {
                        ui.label(
                            RichText::new(&state.usage_label)
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );
                    }
                });
            });
        event
    }
}

/// Lebar sidebar chat (px).
const SIDEBAR_MIN_W: f32 = 280.0;
const SIDEBAR_DEFAULT_W: f32 = 360.0;

/// Frame sidebar: menempel ke tepi kanan, tanpa sudut membulat/bayangan.
fn sidebar_frame() -> egui::Frame {
    egui::Frame {
        inner_margin: egui::Margin::symmetric(10, 8),
        corner_radius: egui::CornerRadius::ZERO,
        shadow: egui::Shadow::NONE,
        ..glass_frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_title_is_humanized() {
        assert_eq!(tool_title("run_command"), "Run command");
        assert_eq!(tool_title("mcp__ducad__run_ops"), "Run ops");
        assert_eq!(tool_title(""), "");
    }

    #[test]
    fn tool_summary_picks_key_argument() {
        assert_eq!(tool_summary(r#"{"CommandLine":"ls -la\nx"}"#), "$ ls -la");
        assert_eq!(
            tool_summary(r#"{"AbsolutePath":"/Users/a/proj/src/main.rs"}"#),
            "…/src/main.rs"
        );
        assert_eq!(tool_summary(r#"{"ops":[{},{}]}"#), "2 op");
        assert_eq!(tool_summary(r#"{"ops":"#), r#"{"ops":"#);
    }
}
