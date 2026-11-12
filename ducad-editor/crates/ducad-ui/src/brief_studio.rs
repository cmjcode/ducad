//! Layar "Proyek baru dari brief": satu layar penuh untuk alur
//! brief → agent bekerja → paket desain → revisi → persetujuan.
//!
//! Tiga fase dalam satu layar:
//! 1. **Brief** — kotak teks besar + lampiran gambar + proses/material/jumlah.
//! 2. **Agent bekerja** — daftar tahap yang hidup dan log aktivitas.
//! 3. **Paket desain** — lembar demi lembar (digambar dari display-list yang
//!    sama dengan PDF), tanda koreksi di lembar, permintaan revisi, dan
//!    gerbang persetujuan.
//!
//! Murni tampilan: caller (`ducad-app`) menjalankan agent, membangun paket,
//! dan menyusun [`BriefStudioData`] tiap frame.

use crate::theme::{
    ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, BG_CARD_DARK, BORDER_SUBTLE, TEXT_MUTED,
    TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_i18n::t;
use ducad_io::drawing::scene::{Anchor, Item, Pen, Scene};
use egui::{
    Align2, Color32, ComboBox, CornerRadius, FontId, Pos2, Rect, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, TextEdit, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_ADD_PHOTO_ALTERNATE, ICON_AUTO_AWESOME, ICON_CHECK_CIRCLE, ICON_CHEVRON_LEFT,
    ICON_CHEVRON_RIGHT, ICON_CLOSE, ICON_DELETE, ICON_ERROR, ICON_PICTURE_AS_PDF,
    ICON_RADIO_BUTTON_UNCHECKED, ICON_SEND, ICON_STOP, ICON_VERIFIED,
};

const FAIL_RED: Color32 = Color32::from_rgb(255, 69, 58);
const MARK_RED: Color32 = Color32::from_rgb(232, 72, 72);
const BG: Color32 = Color32::from_rgb(18, 20, 26);
const PAPER: Color32 = Color32::from_rgb(250, 250, 248);

/// Proses manufaktur yang bisa dipilih di komposer (indeks = urutan combo).
pub const BRIEF_PROCESSES: [(&str, &str); 7] = [
    ("cnc", "CNC"),
    ("sheet_metal", "Sheet metal"),
    ("fdm", "FDM 3D print"),
    ("sla", "SLA 3D print"),
    ("sls", "SLS 3D print"),
    ("injection", "Injection"),
    ("casting", "Casting"),
];

/// Fase layar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BriefPhase {
    #[default]
    Compose,
    Running,
    Result,
}

/// Status satu tahap kerja agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BriefStepState {
    Pending,
    Active,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BriefStepUi {
    pub label: String,
    pub state: BriefStepState,
}

/// Satu baris log aktivitas.
#[derive(Debug, Clone, PartialEq)]
pub struct BriefLogUi {
    /// `None` = teks agent; `Some(ok)` = hasil tool.
    pub ok: Option<bool>,
    pub text: String,
}

/// Tahap rilis paket (sama artinya dengan panel Review).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BriefStage {
    #[default]
    Draft,
    InReview,
    Approved,
}

/// Semua yang ditampilkan layar (disusun caller tiap frame).
#[derive(Debug, Clone, Default)]
pub struct BriefStudioData {
    pub phase: BriefPhase,
    /// Agent sedang berjalan (generasi awal atau revisi).
    pub busy: bool,
    /// Label provider/agent aktif, mis. "Anthropic · claude-…".
    pub agent_label: String,
    /// Alasan agent belum bisa dipakai; `None` = siap.
    pub agent_blocked: Option<String>,
    /// Dokumen sudah berisi model sebelum brief dikirim.
    pub document_has_model: bool,
    pub steps: Vec<BriefStepUi>,
    pub log: Vec<BriefLogUi>,
    /// Judul tiap lembar paket.
    pub pages: Vec<String>,
    /// Display-list lembar yang sedang dipilih.
    pub scene: Option<Scene>,
    pub stage: BriefStage,
    pub revision: String,
    pub approved_by: Option<String>,
    pub blockers: Vec<String>,
    /// Ringkasan: (lulus, gagal, peringatan) check.
    pub checks: (usize, usize, usize),
    /// Ringkasan: (terverifikasi, gagal, tanpa check) persyaratan.
    pub requirements: (usize, usize, usize),
    pub title: String,
    /// Pesan hasil aksi terakhir / error.
    pub status: Option<String>,
    pub error: Option<String>,
    pub attachments: usize,
}

/// Tanda koreksi yang ditaruh pengguna di sebuah lembar.
#[derive(Debug, Clone, PartialEq)]
pub struct BriefMark {
    pub page: usize,
    /// mm dari pojok kiri-bawah kertas.
    pub position: [f32; 2],
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BriefStudioEvent {
    Close,
    Generate,
    AttachImage,
    ClearAttachments,
    Stop,
    /// Kirim permintaan revisi (teks umum + tanda di lembar dari state).
    SendRevision,
    Approve,
    ExportPdf,
    OpenChatSettings,
    /// Kembali ke komposer untuk brief baru.
    NewBrief,
}

/// Keadaan masukan layar (bertahan antar frame).
#[derive(Debug, Clone, Default)]
pub struct BriefStudioState {
    pub open: bool,
    pub brief: String,
    pub process: usize,
    pub material: String,
    pub quantity: String,
    pub page: usize,
    pub marks: Vec<BriefMark>,
    pub revision: String,
    pub approver: String,
}

impl BriefStudioState {
    /// Nama proses terpilih dalam bentuk kontrak engine (`cnc`, …).
    pub fn process_key(&self) -> &'static str {
        BRIEF_PROCESSES
            .get(self.process)
            .map(|p| p.0)
            .unwrap_or("cnc")
    }
}

fn pill(ui: &mut Ui, text: &str, active: bool, done: bool) {
    let color = if active {
        ACCENT_BLUE
    } else if done {
        ACCENT_GREEN
    } else {
        TEXT_MUTED
    };
    ui.label(RichText::new(text).size(12.0).strong().color(color));
}

fn primary_button(ui: &mut Ui, enabled: bool, text: String) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            RichText::new(text)
                .size(14.0)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(ACCENT_BLUE)
        .min_size(Vec2::new(0.0, 34.0)),
    )
}

fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(BG_CARD_DARK)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(12))
        .show(ui, add)
        .inner
}

pub struct BriefStudio;

impl BriefStudio {
    pub fn show(
        ui: &mut Ui,
        state: &mut BriefStudioState,
        data: &BriefStudioData,
    ) -> Option<BriefStudioEvent> {
        let mut event = None;
        let full = ui.available_rect_before_wrap();
        ui.painter().rect_filled(full, CornerRadius::ZERO, BG);
        // Layar ini menutup kanvas: klik di area kosong tidak boleh tembus.
        ui.interact(
            full,
            ui.id().with("brief-studio-bg"),
            Sense::click_and_drag(),
        );

        let header_h = 46.0;
        let header = Rect::from_min_size(full.min, Vec2::new(full.width(), header_h));
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(header.shrink2(Vec2::new(16.0, 8.0))),
            |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(ICON_AUTO_AWESOME.codepoint)
                            .size(18.0)
                            .color(ACCENT_BLUE),
                    );
                    ui.label(
                        RichText::new(t!("brief-title"))
                            .size(16.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.add_space(18.0);
                    let phase = data.phase;
                    pill(
                        ui,
                        &t!("brief-phase-compose"),
                        phase == BriefPhase::Compose,
                        phase != BriefPhase::Compose,
                    );
                    ui.label(RichText::new(ICON_CHEVRON_RIGHT.codepoint).color(TEXT_MUTED));
                    pill(
                        ui,
                        &t!("brief-phase-running"),
                        phase == BriefPhase::Running,
                        phase == BriefPhase::Result,
                    );
                    ui.label(RichText::new(ICON_CHEVRON_RIGHT.codepoint).color(TEXT_MUTED));
                    pill(
                        ui,
                        &t!("brief-phase-result"),
                        phase == BriefPhase::Result,
                        false,
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new(ICON_CLOSE.codepoint).size(16.0))
                            .on_hover_text(t!("brief-close"))
                            .clicked()
                        {
                            event = Some(BriefStudioEvent::Close);
                        }
                        if !data.agent_label.is_empty() {
                            ui.label(RichText::new(&data.agent_label).small().color(TEXT_MUTED));
                        }
                    });
                });
            },
        );
        ui.painter().hline(
            full.x_range(),
            header.max.y,
            Stroke::new(1.0, BORDER_SUBTLE),
        );

        let body = Rect::from_min_max(Pos2::new(full.min.x, header.max.y), full.max).shrink(16.0);
        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
            match data.phase {
                BriefPhase::Compose => Self::compose(ui, state, data, &mut event),
                BriefPhase::Running => Self::running(ui, data, &mut event),
                BriefPhase::Result => Self::result(ui, state, data, &mut event),
            }
        });
        event
    }

    fn compose(
        ui: &mut Ui,
        state: &mut BriefStudioState,
        data: &BriefStudioData,
        event: &mut Option<BriefStudioEvent>,
    ) {
        let width = ui.available_width().min(780.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(width);
            ui.add_space((ui.available_height() * 0.06).min(40.0));
            ui.label(
                RichText::new(t!("brief-compose-heading"))
                    .size(26.0)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            ui.label(RichText::new(t!("brief-compose-sub")).color(TEXT_SECONDARY));
            ui.add_space(14.0);
            card(ui, |ui| {
                ui.set_width(width - 26.0);
                ui.add(
                    TextEdit::multiline(&mut state.brief)
                        .hint_text(t!("brief-compose-hint"))
                        .desired_rows(7)
                        .desired_width(f32::INFINITY)
                        .font(FontId::proportional(15.0)),
                );
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    let label = if data.attachments == 0 {
                        format!(
                            "{} {}",
                            ICON_ADD_PHOTO_ALTERNATE.codepoint,
                            t!("brief-attach")
                        )
                    } else {
                        format!(
                            "{} {}",
                            ICON_ADD_PHOTO_ALTERNATE.codepoint,
                            t!("brief-attached", count = data.attachments)
                        )
                    };
                    if ui
                        .button(label)
                        .on_hover_text(t!("brief-attach-hint"))
                        .clicked()
                    {
                        *event = Some(BriefStudioEvent::AttachImage);
                    }
                    if data.attachments > 0
                        && ui
                            .button(ICON_DELETE.codepoint)
                            .on_hover_text(t!("brief-attach-clear"))
                            .clicked()
                    {
                        *event = Some(BriefStudioEvent::ClearAttachments);
                    }
                    ui.separator();
                    ui.label(RichText::new(t!("brief-process")).color(TEXT_SECONDARY));
                    ComboBox::from_id_salt("ducad-brief-process")
                        .selected_text(BRIEF_PROCESSES[state.process.min(6)].1)
                        .show_ui(ui, |ui| {
                            for (i, (_, label)) in BRIEF_PROCESSES.iter().enumerate() {
                                ui.selectable_value(&mut state.process, i, *label);
                            }
                        });
                    ui.label(RichText::new(t!("brief-material")).color(TEXT_SECONDARY));
                    ui.add(
                        TextEdit::singleline(&mut state.material)
                            .hint_text("Al 6061-T6")
                            .desired_width(120.0),
                    );
                    ui.label(RichText::new(t!("brief-quantity")).color(TEXT_SECONDARY));
                    ui.add(
                        TextEdit::singleline(&mut state.quantity)
                            .hint_text("1")
                            .desired_width(50.0),
                    );
                });
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(t!("brief-examples"))
                        .small()
                        .color(TEXT_MUTED),
                );
                for key in ["brief-example-1", "brief-example-2", "brief-example-3"] {
                    let text = ducad_i18n::translate(key, None);
                    let short: String = text.chars().take(38).collect();
                    if ui
                        .small_button(format!("{short}…"))
                        .on_hover_text(text.as_str())
                        .clicked()
                    {
                        state.brief = text;
                    }
                }
            });
            ui.add_space(10.0);
            if let Some(reason) = &data.agent_blocked {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(ICON_ERROR.codepoint).color(ACCENT_ORANGE));
                    ui.label(RichText::new(reason).color(ACCENT_ORANGE));
                    if ui.button(t!("brief-open-chat-settings")).clicked() {
                        *event = Some(BriefStudioEvent::OpenChatSettings);
                    }
                });
            }
            if data.document_has_model {
                ui.label(
                    RichText::new(t!("brief-document-has-model"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
            }
            if let Some(error) = &data.error {
                ui.label(RichText::new(error).color(FAIL_RED));
            }
            ui.add_space(6.0);
            let ready = !state.brief.trim().is_empty() && data.agent_blocked.is_none();
            if primary_button(
                ui,
                ready,
                format!("{}  {}", ICON_AUTO_AWESOME.codepoint, t!("brief-generate")),
            )
            .clicked()
            {
                *event = Some(BriefStudioEvent::Generate);
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(t!("brief-compose-note"))
                    .small()
                    .color(TEXT_MUTED),
            );
        });
    }

    fn steps(ui: &mut Ui, data: &BriefStudioData) {
        for step in &data.steps {
            let (icon, color) = match step.state {
                BriefStepState::Done => (ICON_CHECK_CIRCLE, ACCENT_GREEN),
                BriefStepState::Active => (ICON_AUTO_AWESOME, ACCENT_BLUE),
                BriefStepState::Failed => (ICON_ERROR, FAIL_RED),
                BriefStepState::Pending => (ICON_RADIO_BUTTON_UNCHECKED, TEXT_MUTED),
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon.codepoint).size(16.0).color(color));
                let text = RichText::new(&step.label).size(14.0);
                ui.label(match step.state {
                    BriefStepState::Pending => text.color(TEXT_MUTED),
                    BriefStepState::Active => text.strong().color(TEXT_PRIMARY),
                    _ => text.color(TEXT_PRIMARY),
                });
                if step.state == BriefStepState::Active {
                    ui.spinner();
                }
            });
        }
    }

    fn log(ui: &mut Ui, data: &BriefStudioData, max_height: f32) {
        ScrollArea::vertical()
            .id_salt("brief-log")
            .max_height(max_height)
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in &data.log {
                    match line.ok {
                        None => {
                            ui.label(RichText::new(&line.text).color(TEXT_PRIMARY));
                        }
                        Some(ok) => {
                            ui.horizontal_wrapped(|ui| {
                                let (icon, color) = if ok {
                                    (ICON_CHECK_CIRCLE, ACCENT_GREEN)
                                } else {
                                    (ICON_ERROR, ACCENT_ORANGE)
                                };
                                ui.label(RichText::new(icon.codepoint).color(color));
                                ui.label(RichText::new(&line.text).small().color(TEXT_SECONDARY));
                            });
                        }
                    }
                }
            });
    }

    fn running(ui: &mut Ui, data: &BriefStudioData, event: &mut Option<BriefStudioEvent>) {
        let full = ui.available_rect_before_wrap();
        let left = Rect::from_min_size(full.min, Vec2::new(300.0, full.height()));
        let right = Rect::from_min_max(Pos2::new(left.max.x + 12.0, full.min.y), full.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
            card(ui, |ui| {
                ui.set_width(274.0);
                ui.label(
                    RichText::new(t!("brief-running-heading"))
                        .size(18.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(t!("brief-running-sub"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(10.0);
                Self::steps(ui, data);
                ui.add_space(12.0);
                if ui
                    .button(format!("{} {}", ICON_STOP.codepoint, t!("brief-stop")))
                    .clicked()
                {
                    *event = Some(BriefStudioEvent::Stop);
                }
            });
        });
        ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| {
            ui.label(
                RichText::new(t!("brief-activity"))
                    .strong()
                    .color(TEXT_SECONDARY),
            );
            card(ui, |ui| {
                ui.set_width(right.width() - 26.0);
                Self::log(ui, data, (full.height() - 70.0).max(120.0));
            });
        });
    }

    fn result(
        ui: &mut Ui,
        state: &mut BriefStudioState,
        data: &BriefStudioData,
        event: &mut Option<BriefStudioEvent>,
    ) {
        let height = ui.available_height();
        // Dijepit hanya bila lembarnya sudah ada: selagi paket dibangun
        // (daftar kosong) pilihan lembar tidak boleh kembali ke sampul.
        if !data.pages.is_empty() && state.page >= data.pages.len() {
            state.page = data.pages.len() - 1;
        }
        // Tiga kolom dengan kotak eksplisit: tata letak mengalir membuat
        // kolom lembar mendorong kolom revisi keluar layar.
        let full = ui.available_rect_before_wrap();
        let side = 320.0f32.min(full.width() * 0.3);
        let left_rect = Rect::from_min_size(full.min, Vec2::new(230.0, height));
        let side_rect = Rect::from_min_size(
            Pos2::new(full.max.x - side, full.min.y),
            Vec2::new(side, height),
        );
        let center_rect = Rect::from_min_max(
            Pos2::new(left_rect.max.x + 10.0, full.min.y),
            Pos2::new(side_rect.min.x - 10.0, full.max.y),
        );
        let center_w = center_rect.width().max(120.0);
        {
            // Kiri: daftar lembar + ringkasan.
            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(230.0);
                    ui.label(
                        RichText::new(&data.title)
                            .size(16.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    let (stage, color) = match data.stage {
                        BriefStage::Draft => (t!("review-stage-draft"), TEXT_SECONDARY),
                        BriefStage::InReview => (t!("review-stage-in-review"), ACCENT_ORANGE),
                        BriefStage::Approved => (t!("review-stage-approved"), ACCENT_GREEN),
                    };
                    ui.label(
                        RichText::new(format!("{stage} · rev {}", data.revision))
                            .small()
                            .color(color),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(t!("brief-sheets"))
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    ScrollArea::vertical()
                        .id_salt("brief-sheets")
                        .max_height((height - 290.0).max(100.0))
                        .show(ui, |ui| {
                            for (i, title) in data.pages.iter().enumerate() {
                                let marks = state.marks.iter().filter(|m| m.page == i).count();
                                let label = if marks > 0 {
                                    format!("{}. {title}  ({marks})", i + 1)
                                } else {
                                    format!("{}. {title}", i + 1)
                                };
                                if ui.selectable_label(state.page == i, label).clicked() {
                                    state.page = i;
                                }
                            }
                        });
                    ui.add_space(8.0);
                    card(ui, |ui| {
                        ui.set_width(204.0);
                        let (pass, fail, warn) = data.checks;
                        ui.label(
                            RichText::new(t!(
                                "brief-summary-checks",
                                pass = pass,
                                fail = fail,
                                warn = warn
                            ))
                            .small()
                            .color(if fail == 0 {
                                ACCENT_GREEN
                            } else {
                                FAIL_RED
                            }),
                        );
                        let (ok, bad, none) = data.requirements;
                        ui.label(
                            RichText::new(t!(
                                "brief-summary-requirements",
                                ok = ok,
                                bad = bad,
                                none = none
                            ))
                            .small()
                            .color(if bad == 0 {
                                TEXT_PRIMARY
                            } else {
                                FAIL_RED
                            }),
                        );
                    });
                    ui.add_space(8.0);
                    if ui
                        .button(format!(
                            "{} {}",
                            ICON_PICTURE_AS_PDF.codepoint,
                            t!("brief-export-pdf")
                        ))
                        .clicked()
                    {
                        *event = Some(BriefStudioEvent::ExportPdf);
                    }
                    if ui
                        .button(t!("brief-open-editor"))
                        .on_hover_text(t!("brief-open-editor-hint"))
                        .clicked()
                    {
                        *event = Some(BriefStudioEvent::Close);
                    }
                    if ui.button(t!("brief-new")).clicked() {
                        *event = Some(BriefStudioEvent::NewBrief);
                    }
                });
            });

            // Tengah: lembar.
            ui.scope_builder(egui::UiBuilder::new().max_rect(center_rect), |ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(center_w);
                    ui.horizontal(|ui| {
                        let n = data.pages.len();
                        if ui
                            .add_enabled(
                                state.page > 0,
                                egui::Button::new(ICON_CHEVRON_LEFT.codepoint),
                            )
                            .clicked()
                        {
                            state.page -= 1;
                        }
                        ui.label(
                            RichText::new(format!(
                                "{} / {}  {}",
                                (state.page + 1).min(n.max(1)),
                                n,
                                data.pages.get(state.page).map(String::as_str).unwrap_or("")
                            ))
                            .color(TEXT_PRIMARY),
                        );
                        if ui
                            .add_enabled(
                                state.page + 1 < n,
                                egui::Button::new(ICON_CHEVRON_RIGHT.codepoint),
                            )
                            .clicked()
                        {
                            state.page += 1;
                        }
                        ui.label(
                            RichText::new(t!("brief-mark-hint"))
                                .small()
                                .color(TEXT_MUTED),
                        );
                    });
                    // Lebar dikunci ke kolom tengah: `available_size` bisa
                    // melebar dan mendorong kolom revisi keluar layar.
                    let avail = Vec2::new(center_w, ui.available_height().max(120.0));
                    let (rect, response) = ui.allocate_exact_size(avail, Sense::click());
                    if let Some(scene) = &data.scene {
                        let view = SceneView::fit(scene, rect.shrink(4.0));
                        view.paint(ui, scene);
                        for (i, mark) in state.marks.iter().enumerate() {
                            if mark.page != state.page {
                                continue;
                            }
                            let p = view.to_screen(mark.position);
                            ui.painter()
                                .circle(p, 9.0, MARK_RED, Stroke::new(1.5, Color32::WHITE));
                            ui.painter().text(
                                p,
                                Align2::CENTER_CENTER,
                                (i + 1).to_string(),
                                FontId::proportional(11.0),
                                Color32::WHITE,
                            );
                        }
                        if response.clicked() && !data.busy {
                            if let Some(pos) = response.interact_pointer_pos() {
                                if view.paper.contains(pos) {
                                    state.marks.push(BriefMark {
                                        page: state.page,
                                        position: view.to_mm(pos),
                                        text: String::new(),
                                    });
                                }
                            }
                        }
                    } else {
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            t!("brief-no-pages"),
                            FontId::proportional(14.0),
                            TEXT_MUTED,
                        );
                    }
                });
            });

            // Kanan: revisi + persetujuan.
            ui.scope_builder(egui::UiBuilder::new().max_rect(side_rect), |ui| {
                ScrollArea::vertical().id_salt("brief-side").show(ui, |ui| {
                    ui.set_width(side - 12.0);
                    if data.busy {
                        card(ui, |ui| {
                            ui.set_width(side - 40.0);
                            ui.label(
                                RichText::new(t!("brief-revising"))
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            Self::steps(ui, data);
                            ui.add_space(6.0);
                            if ui
                                .button(format!("{} {}", ICON_STOP.codepoint, t!("brief-stop")))
                                .clicked()
                            {
                                *event = Some(BriefStudioEvent::Stop);
                            }
                        });
                        ui.add_space(8.0);
                    }
                    ui.label(
                        RichText::new(t!("brief-revise-heading"))
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    let mut remove = None;
                    for (i, mark) in state.marks.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{}", i + 1)).strong().color(MARK_RED));
                            ui.add(
                                TextEdit::singleline(&mut mark.text)
                                    .hint_text(t!("brief-mark-text-hint", sheet = mark.page + 1))
                                    .desired_width(side - 100.0),
                            );
                            if ui.small_button(ICON_DELETE.codepoint).clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        state.marks.remove(i);
                    }
                    ui.add(
                        TextEdit::multiline(&mut state.revision)
                            .hint_text(t!("brief-revise-hint"))
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    ui.horizontal(|ui| {
                        let has_marks = state.marks.iter().any(|m| !m.text.trim().is_empty());
                        let ready = !data.busy
                            && data.agent_blocked.is_none()
                            && (!state.revision.trim().is_empty() || has_marks);
                        if primary_button(
                            ui,
                            ready,
                            format!("{} {}", ICON_SEND.codepoint, t!("brief-revise-send")),
                        )
                        .clicked()
                        {
                            *event = Some(BriefStudioEvent::SendRevision);
                        }
                        let attach = if data.attachments == 0 {
                            ICON_ADD_PHOTO_ALTERNATE.codepoint.to_string()
                        } else {
                            format!(
                                "{} {}",
                                ICON_ADD_PHOTO_ALTERNATE.codepoint, data.attachments
                            )
                        };
                        if ui
                            .button(attach)
                            .on_hover_text(t!("brief-attach-hint"))
                            .clicked()
                        {
                            *event = Some(BriefStudioEvent::AttachImage);
                        }
                    });
                    if let Some(reason) = &data.agent_blocked {
                        ui.label(RichText::new(reason).small().color(ACCENT_ORANGE));
                    }
                    if let Some(error) = &data.error {
                        ui.label(RichText::new(error).small().color(FAIL_RED));
                    }

                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(t!("brief-approve-heading"))
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    if let Some(by) = &data.approved_by {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(ICON_VERIFIED.codepoint).color(ACCENT_GREEN));
                            ui.label(
                                RichText::new(t!("review-approved-by", name = by.as_str()))
                                    .color(TEXT_PRIMARY),
                            );
                        });
                    } else {
                        for b in &data.blockers {
                            ui.label(RichText::new(b).small().color(FAIL_RED));
                        }
                        ui.add(
                            TextEdit::singleline(&mut state.approver)
                                .hint_text(t!("review-by-hint"))
                                .desired_width(f32::INFINITY),
                        );
                        let can = !data.busy
                            && data.blockers.is_empty()
                            && !state.approver.trim().is_empty();
                        if ui
                            .add_enabled(
                                can,
                                egui::Button::new(format!(
                                    "{} {}",
                                    ICON_VERIFIED.codepoint,
                                    t!("brief-approve")
                                )),
                            )
                            .on_disabled_hover_text(t!("review-approve-disabled"))
                            .clicked()
                        {
                            *event = Some(BriefStudioEvent::Approve);
                        }
                    }
                    ui.label(
                        RichText::new(t!("review-disclaimer"))
                            .small()
                            .color(TEXT_MUTED),
                    );
                    if let Some(status) = &data.status {
                        ui.add_space(6.0);
                        ui.label(RichText::new(status).small().color(TEXT_SECONDARY));
                    }

                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(t!("brief-activity"))
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    card(ui, |ui| {
                        ui.set_width(side - 40.0);
                        Self::log(ui, data, 220.0);
                    });
                });
            });
        }
    }
}

/// Pemetaan mm kertas (Y ke atas) ↔ layar untuk satu lembar.
#[derive(Debug, Clone, Copy)]
pub struct SceneView {
    /// Kotak kertas di layar.
    pub paper: Rect,
    /// Piksel per mm.
    pub scale: f32,
    height_mm: f32,
}

impl SceneView {
    /// Lembar dipaskan dan dipusatkan di `rect`.
    pub fn fit(scene: &Scene, rect: Rect) -> Self {
        let scale = (rect.width() / scene.width_mm.max(1.0))
            .min(rect.height() / scene.height_mm.max(1.0))
            .max(0.01);
        let size = Vec2::new(scene.width_mm * scale, scene.height_mm * scale);
        Self {
            paper: Rect::from_center_size(rect.center(), size),
            scale,
            height_mm: scene.height_mm,
        }
    }

    pub fn to_screen(&self, p: [f32; 2]) -> Pos2 {
        Pos2::new(
            self.paper.min.x + p[0] * self.scale,
            self.paper.min.y + (self.height_mm - p[1]) * self.scale,
        )
    }

    pub fn to_mm(&self, p: Pos2) -> [f32; 2] {
        [
            (p.x - self.paper.min.x) / self.scale,
            self.height_mm - (p.y - self.paper.min.y) / self.scale,
        ]
    }

    fn stroke(&self, pen: &Pen) -> Stroke {
        Stroke::new(
            (pen.width_mm * self.scale).clamp(0.6, 3.0),
            Color32::from_rgb(pen.color[0], pen.color[1], pen.color[2]),
        )
    }

    fn line(&self, ui: &Ui, a: Pos2, b: Pos2, pen: &Pen) {
        let stroke = self.stroke(pen);
        let pattern = pen.dash.pattern_mm();
        if pattern.is_empty() {
            ui.painter().line_segment([a, b], stroke);
            return;
        }
        let dashes: Vec<f32> = pattern
            .iter()
            .step_by(2)
            .map(|d| (d * self.scale).max(1.0))
            .collect();
        let gaps: Vec<f32> = pattern
            .iter()
            .skip(1)
            .step_by(2)
            .map(|d| (d * self.scale).max(1.0))
            .collect();
        ui.painter().extend(egui::Shape::dashed_line_with_offset(
            &[a, b],
            stroke,
            &dashes,
            &gaps,
            0.0,
        ));
    }

    /// Gambar display-list lembar (primitif yang sama dengan ekspor PDF).
    pub fn paint(&self, ui: &Ui, scene: &Scene) {
        let painter = ui.painter();
        painter.rect_filled(self.paper, CornerRadius::ZERO, PAPER);
        let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
        for group in &scene.groups {
            for item in &group.items {
                match item {
                    Item::Line { a, b, pen } => {
                        self.line(ui, self.to_screen(*a), self.to_screen(*b), pen)
                    }
                    Item::Arc {
                        center,
                        radius,
                        start_deg,
                        end_deg,
                        pen,
                    } => {
                        let sweep = (end_deg - start_deg).clamp(0.0, 360.0);
                        let n = ((sweep / 6.0).ceil() as usize).max(2);
                        let points: Vec<Pos2> = (0..=n)
                            .map(|k| {
                                let a = (start_deg + sweep * k as f32 / n as f32).to_radians();
                                self.to_screen([
                                    center[0] + radius * a.cos(),
                                    center[1] + radius * a.sin(),
                                ])
                            })
                            .collect();
                        painter.add(egui::Shape::line(points, self.stroke(pen)));
                    }
                    Item::Circle {
                        center,
                        radius,
                        pen,
                        fill,
                    } => {
                        let c = self.to_screen(*center);
                        let r = radius * self.scale;
                        if let Some(fill) = fill {
                            painter.circle_filled(c, r, rgb(*fill));
                        }
                        painter.circle_stroke(c, r, self.stroke(pen));
                    }
                    Item::Rect {
                        min,
                        max,
                        pen,
                        fill,
                    } => {
                        let rect = Rect::from_two_pos(self.to_screen(*min), self.to_screen(*max));
                        if let Some(fill) = fill {
                            painter.rect_filled(rect, CornerRadius::ZERO, rgb(*fill));
                        }
                        if let Some(pen) = pen {
                            painter.rect_stroke(
                                rect,
                                CornerRadius::ZERO,
                                self.stroke(pen),
                                StrokeKind::Middle,
                            );
                        }
                    }
                    Item::Fill { points, color, .. } => {
                        if points.len() >= 3 {
                            painter.add(egui::Shape::convex_polygon(
                                points.iter().map(|p| self.to_screen(*p)).collect(),
                                rgb(*color),
                                Stroke::NONE,
                            ));
                        }
                    }
                    Item::Text {
                        pos,
                        text,
                        size_mm,
                        bold,
                        anchor,
                        angle_deg,
                        ..
                    } => {
                        // Tinggi huruf di bawah ~4 px tidak terbaca: lewati
                        // supaya lembar yang diperkecil tidak menjadi noda.
                        let px = size_mm * self.scale * 1.25;
                        if px < 4.0 {
                            continue;
                        }
                        let color = if *bold {
                            Color32::BLACK
                        } else {
                            Color32::from_rgb(25, 25, 28)
                        };
                        let mut galley =
                            painter.layout_no_wrap(text.clone(), FontId::proportional(px), color);
                        // Tata letak lembar dihitung dengan lebar huruf PDF
                        // (Helvetica). Font aplikasi lebih lebar: kecilkan
                        // supaya teks tidak melewati sel/kolomnya.
                        let expected =
                            ducad_io::drawing::gdt::text_width_mm(text, *size_mm) * self.scale;
                        let mut px = px;
                        if expected > 1.0 && galley.size().x > expected * 1.02 {
                            px *= expected / galley.size().x;
                            if px < 3.5 {
                                continue;
                            }
                            galley = painter.layout_no_wrap(
                                text.clone(),
                                FontId::proportional(px),
                                color,
                            );
                        }
                        let w = galley.size().x;
                        let shift = match anchor {
                            Anchor::Start => 0.0,
                            Anchor::Middle => -0.5 * w,
                            Anchor::End => -w,
                        };
                        // `TextShape::pos` = pojok kiri-atas galley SEBELUM
                        // rotasi; garis dasar ±80 % tinggi huruf di bawahnya.
                        let ascent = px * 0.80;
                        let (sin, cos) = (-angle_deg.to_radians()).sin_cos();
                        let base = self.to_screen(*pos);
                        let origin = Pos2::new(
                            base.x + shift * cos + ascent * sin,
                            base.y + shift * sin - ascent * cos,
                        );
                        let mut shape = egui::epaint::TextShape::new(origin, galley, color);
                        shape.angle = -angle_deg.to_radians();
                        painter.add(shape);
                    }
                    Item::Image { min, max, .. } => {
                        let rect = Rect::from_two_pos(self.to_screen(*min), self.to_screen(*max));
                        painter.rect_filled(
                            rect,
                            CornerRadius::ZERO,
                            Color32::from_rgb(232, 234, 238),
                        );
                    }
                }
            }
        }
        painter.rect_stroke(
            self.paper,
            CornerRadius::ZERO,
            Stroke::new(1.0, Color32::from_rgb(70, 74, 84)),
            StrokeKind::Outside,
        );
    }
}
