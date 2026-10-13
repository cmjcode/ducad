//! Panel "Review & Rilis" (W0–W5): brief desain, keterlacakan persyaratan,
//! ceklis review, redline, tabel revisi, dan gerbang persetujuan paket.
//!
//! Panel ini murni tampilan: caller (`ducad-app`) menyusun
//! [`ReviewPanelData`] dari `DesignDoc` dan menangani [`ReviewPanelEvent`].
//! Persetujuan hanya bisa datang dari sini (pengguna) — agent tidak punya
//! tool untuk menyetujui paket pada sesi live.

use std::collections::BTreeMap;

use crate::theme::{
    glass_frame, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{Color32, RichText, ScrollArea, TextEdit, Ui, Vec2};
use egui_icons::icons::{
    ICON_CHECK_CIRCLE, ICON_CLOSE, ICON_ERROR, ICON_HELP, ICON_PICTURE_AS_PDF, ICON_RATE_REVIEW,
    ICON_VERIFIED,
};

const FAIL_RED: Color32 = Color32::from_rgb(255, 69, 58);
const REDLINE: Color32 = Color32::from_rgb(232, 72, 72);
const PANEL_W: f32 = 380.0;
/// Tinggi yang disisakan untuk bagian Rilis + pesan status di bawah area gulir.
const RELEASE_RESERVED: f32 = 300.0;

/// Status keterlacakan satu persyaratan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewTrace {
    Verified,
    Failed,
    Unverified,
}

/// Tahap rilis paket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReviewStage {
    #[default]
    Draft,
    InReview,
    Approved,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewRequirementUi {
    pub id: String,
    pub text: String,
    pub status: ReviewTrace,
    /// Check yang memverifikasinya, dipisah koma.
    pub checks: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewItemUi {
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewRedlineUi {
    pub sheet: String,
    pub id: String,
    pub text: String,
    pub author: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewRevisionUi {
    pub revision: String,
    pub date: String,
    pub by: String,
    pub description: String,
}

/// Semua yang ditampilkan panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReviewPanelData {
    pub has_brief: bool,
    pub title: String,
    pub function: String,
    /// Proses manufaktur dari brief (kosong bila belum diisi).
    pub process: String,
    pub requirements: Vec<ReviewRequirementUi>,
    pub stage: ReviewStage,
    pub revision: String,
    pub approved_by: Option<String>,
    /// Persetujuan tercatat tetapi desain sudah berubah sejak itu.
    pub approval_stale: bool,
    pub checklist: Vec<ReviewItemUi>,
    /// Alasan paket belum boleh disetujui.
    pub blockers: Vec<String>,
    pub redlines: Vec<ReviewRedlineUi>,
    pub history: Vec<ReviewRevisionUi>,
    /// Titik redline sudah diklik di lembar; panel menunggu teksnya.
    pub pending_redline: bool,
    /// Pesan hasil aksi terakhir.
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReviewPanelEvent {
    Close,
    /// Simpan judul/fungsi brief.
    SaveBrief {
        title: String,
        function: String,
    },
    AddRequirement {
        text: String,
    },
    Submit {
        description: String,
        by: String,
    },
    Approve {
        by: String,
    },
    Reopen,
    BuildPackage,
    /// Aktifkan alat redline di lembar gambar.
    StartRedline,
    AddRedline {
        text: String,
    },
    CancelRedline,
    ResolveRedline {
        sheet: String,
        id: String,
        resolution: String,
    },
}

/// Keadaan masukan panel (bertahan antar frame).
#[derive(Debug, Clone, Default)]
pub struct ReviewPanel {
    pub by: String,
    pub description: String,
    pub redline_text: String,
    pub new_requirement: String,
    /// Kunci `lembar/id` → teks penyelesaian yang sedang diketik.
    pub resolutions: BTreeMap<String, String>,
    title: String,
    function: String,
    /// Brief asal isian `title`/`function` (deteksi dokumen berganti).
    loaded: Option<(String, String)>,
}

fn section(ui: &mut Ui, text: String) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(text)
            .strong()
            .size(12.0)
            .color(TEXT_SECONDARY),
    );
}

impl ReviewPanel {
    pub fn show(&mut self, ui: &mut Ui, data: &ReviewPanelData) -> Option<ReviewPanelEvent> {
        let mut event = None;
        // Isian brief mengikuti dokumen selama pengguna belum mengubahnya.
        let source = (data.title.clone(), data.function.clone());
        if self.loaded.as_ref() != Some(&source) {
            self.title = data.title.clone();
            self.function = data.function.clone();
            self.loaded = Some(source);
        }
        glass_frame().show(ui, |ui| {
            ui.set_width(PANEL_W);
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 5.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(ICON_RATE_REVIEW.codepoint).color(ACCENT_BLUE));
                ui.label(
                    RichText::new(t!("review-title"))
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                let (stage, color) = match data.stage {
                    ReviewStage::Draft => (t!("review-stage-draft"), TEXT_SECONDARY),
                    ReviewStage::InReview => (t!("review-stage-in-review"), ACCENT_ORANGE),
                    ReviewStage::Approved => (t!("review-stage-approved"), ACCENT_GREEN),
                };
                ui.label(
                    RichText::new(format!("{stage} · rev {}", data.revision))
                        .small()
                        .color(color),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(ICON_CLOSE.codepoint)
                        .on_hover_text(t!("review-close"))
                        .clicked()
                    {
                        event = Some(ReviewPanelEvent::Close);
                    }
                });
            });
            ui.separator();
            // Bagian Rilis (ajukan/setujui/bangun paket) disematkan di bawah
            // area gulir supaya tombolnya selalu terlihat, juga di layar
            // pendek (iPad lanskap).
            let screen_h = ui.ctx().content_rect().height();
            let scroll_h = (screen_h - ui.cursor().top() - RELEASE_RESERVED).clamp(140.0, 520.0);
            ScrollArea::vertical().max_height(scroll_h).show(ui, |ui| {
                self.brief(ui, data, &mut event);
                self.requirements(ui, data, &mut event);
                Self::checklist(ui, data);
                self.redlines(ui, data, &mut event);
                Self::history(ui, data);
            });
            ui.separator();
            self.release(ui, data, &mut event);
            if let Some(status) = &data.status {
                ui.separator();
                ui.label(RichText::new(status).small().color(TEXT_SECONDARY));
            }
        });
        event
    }

    fn brief(&mut self, ui: &mut Ui, data: &ReviewPanelData, event: &mut Option<ReviewPanelEvent>) {
        section(ui, t!("review-brief"));
        if !data.has_brief {
            ui.label(
                RichText::new(t!("review-brief-empty"))
                    .small()
                    .color(TEXT_MUTED),
            );
        }
        ui.add(
            TextEdit::singleline(&mut self.title)
                .hint_text(t!("review-brief-title-hint"))
                .desired_width(f32::INFINITY),
        );
        ui.add(
            TextEdit::multiline(&mut self.function)
                .hint_text(t!("review-brief-function-hint"))
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            let dirty = self.title != data.title || self.function != data.function;
            if ui
                .add_enabled(dirty, egui::Button::new(t!("review-brief-save")))
                .clicked()
            {
                *event = Some(ReviewPanelEvent::SaveBrief {
                    title: self.title.trim().to_string(),
                    function: self.function.trim().to_string(),
                });
            }
            if !data.process.is_empty() {
                ui.label(
                    RichText::new(t!("review-process", process = data.process.as_str()))
                        .small()
                        .color(TEXT_SECONDARY),
                );
            }
        });
    }

    fn requirements(
        &mut self,
        ui: &mut Ui,
        data: &ReviewPanelData,
        event: &mut Option<ReviewPanelEvent>,
    ) {
        section(ui, t!("review-requirements"));
        for req in &data.requirements {
            let (icon, color, hint) = match req.status {
                ReviewTrace::Verified => {
                    (ICON_CHECK_CIRCLE, ACCENT_GREEN, t!("review-trace-verified"))
                }
                ReviewTrace::Failed => (ICON_ERROR, FAIL_RED, t!("review-trace-failed")),
                ReviewTrace::Unverified => {
                    (ICON_HELP, ACCENT_ORANGE, t!("review-trace-unverified"))
                }
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon.codepoint).color(color))
                    .on_hover_text(hint);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!("{}  {}", req.id, req.text)).color(TEXT_PRIMARY),
                    );
                    if !req.checks.is_empty() {
                        ui.label(RichText::new(&req.checks).small().color(TEXT_SECONDARY));
                    }
                });
            });
        }
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut self.new_requirement)
                    .hint_text(t!("review-requirement-hint"))
                    .desired_width(PANEL_W - 110.0),
            );
            let ready = !self.new_requirement.trim().is_empty();
            if ui
                .add_enabled(ready, egui::Button::new(t!("review-requirement-add")))
                .clicked()
            {
                *event = Some(ReviewPanelEvent::AddRequirement {
                    text: std::mem::take(&mut self.new_requirement).trim().to_string(),
                });
            }
        });
    }

    fn checklist(ui: &mut Ui, data: &ReviewPanelData) {
        section(ui, t!("review-checklist"));
        for item in &data.checklist {
            let (icon, color) = if item.ok {
                (ICON_CHECK_CIRCLE, ACCENT_GREEN)
            } else {
                (ICON_ERROR, ACCENT_ORANGE)
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon.codepoint).color(color));
                ui.label(RichText::new(&item.label).color(TEXT_PRIMARY));
                ui.label(RichText::new(&item.detail).small().color(TEXT_SECONDARY));
            });
        }
    }

    fn redlines(
        &mut self,
        ui: &mut Ui,
        data: &ReviewPanelData,
        event: &mut Option<ReviewPanelEvent>,
    ) {
        section(ui, t!("review-redlines", count = data.redlines.len()));
        for r in &data.redlines {
            let key = format!("{}/{}", r.sheet, r.id);
            ui.horizontal(|ui| {
                ui.label(RichText::new(&r.id).strong().color(REDLINE));
                ui.label(RichText::new(&r.text).color(TEXT_PRIMARY));
            });
            ui.horizontal(|ui| {
                let text = self.resolutions.entry(key.clone()).or_default();
                ui.add(
                    TextEdit::singleline(text)
                        .hint_text(t!("review-redline-resolution-hint"))
                        .desired_width(PANEL_W - 120.0),
                );
                let ready = !text.trim().is_empty();
                if ui
                    .add_enabled(ready, egui::Button::new(t!("review-redline-resolve")))
                    .clicked()
                {
                    *event = Some(ReviewPanelEvent::ResolveRedline {
                        sheet: r.sheet.clone(),
                        id: r.id.clone(),
                        resolution: text.trim().to_string(),
                    });
                }
            });
        }
        if event
            .as_ref()
            .is_some_and(|e| matches!(e, ReviewPanelEvent::ResolveRedline { .. }))
        {
            if let Some(ReviewPanelEvent::ResolveRedline { sheet, id, .. }) = event.as_ref() {
                self.resolutions.remove(&format!("{sheet}/{id}"));
            }
        }
        if data.pending_redline {
            ui.label(
                RichText::new(t!("review-redline-pending"))
                    .small()
                    .color(REDLINE),
            );
            ui.add(
                TextEdit::multiline(&mut self.redline_text)
                    .hint_text(t!("review-redline-text-hint"))
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                let ready = !self.redline_text.trim().is_empty();
                if ui
                    .add_enabled(ready, egui::Button::new(t!("review-redline-add")))
                    .clicked()
                {
                    *event = Some(ReviewPanelEvent::AddRedline {
                        text: std::mem::take(&mut self.redline_text).trim().to_string(),
                    });
                }
                if ui.button(t!("review-cancel")).clicked() {
                    self.redline_text.clear();
                    *event = Some(ReviewPanelEvent::CancelRedline);
                }
            });
        } else if ui
            .button(t!("review-redline-start"))
            .on_hover_text(t!("review-redline-start-hint"))
            .clicked()
        {
            *event = Some(ReviewPanelEvent::StartRedline);
        }
    }

    fn release(
        &mut self,
        ui: &mut Ui,
        data: &ReviewPanelData,
        event: &mut Option<ReviewPanelEvent>,
    ) {
        section(ui, t!("review-release"));
        if data.approval_stale {
            ui.label(
                RichText::new(t!("review-approval-stale"))
                    .small()
                    .color(ACCENT_ORANGE),
            );
        }
        if let Some(by) = &data.approved_by {
            ui.horizontal(|ui| {
                ui.label(RichText::new(ICON_VERIFIED.codepoint).color(ACCENT_GREEN));
                ui.label(
                    RichText::new(t!("review-approved-by", name = by.as_str())).color(TEXT_PRIMARY),
                );
            });
        }
        for b in &data.blockers {
            ui.label(RichText::new(b).small().color(FAIL_RED));
        }
        ui.add(
            TextEdit::singleline(&mut self.by)
                .hint_text(t!("review-by-hint"))
                .desired_width(f32::INFINITY),
        );
        ui.add(
            TextEdit::singleline(&mut self.description)
                .hint_text(t!("review-description-hint"))
                .desired_width(f32::INFINITY),
        );
        ui.horizontal_wrapped(|ui| {
            let approved = data.stage == ReviewStage::Approved;
            if ui
                .add_enabled(!approved, egui::Button::new(t!("review-submit")))
                .clicked()
            {
                *event = Some(ReviewPanelEvent::Submit {
                    description: std::mem::take(&mut self.description).trim().to_string(),
                    by: self.by.trim().to_string(),
                });
            }
            let can_approve = !approved && data.blockers.is_empty() && !self.by.trim().is_empty();
            if ui
                .add_enabled(can_approve, egui::Button::new(t!("review-approve")))
                .on_disabled_hover_text(t!("review-approve-disabled"))
                .clicked()
            {
                *event = Some(ReviewPanelEvent::Approve {
                    by: self.by.trim().to_string(),
                });
            }
            if approved && ui.button(t!("review-reopen")).clicked() {
                *event = Some(ReviewPanelEvent::Reopen);
            }
        });
        if ui
            .button(format!(
                "{} {}",
                ICON_PICTURE_AS_PDF.codepoint,
                t!("review-build-package")
            ))
            .on_hover_text(t!("review-build-package-hint"))
            .clicked()
        {
            *event = Some(ReviewPanelEvent::BuildPackage);
        }
        ui.label(
            RichText::new(t!("review-disclaimer"))
                .small()
                .color(TEXT_MUTED),
        );
    }

    fn history(ui: &mut Ui, data: &ReviewPanelData) {
        if data.history.is_empty() {
            return;
        }
        section(ui, t!("review-history"));
        for h in &data.history {
            ui.label(
                RichText::new(format!(
                    "{}  {}  {}  {}",
                    h.revision, h.date, h.by, h.description
                ))
                .small()
                .color(TEXT_SECONDARY),
            );
        }
    }
}
