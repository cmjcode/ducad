//! Datum Drawer — Panel daftar geometri referensi 3D bergaya Shapr3D.
//!
//! Menampilkan bidang referensi (Top, Front, Right, serta Datum Plane kustom),
//! sumbu datum, dan titik datum (Paritas Fase 1A butir 1.5) di pojok kanan
//! bawah kanvas: badge status aktif, ganti bidang instan, buat datum baru,
//! tampil/sembunyi, ganti nama (klik ganda / tombol edit), dan hapus.

use crate::datum_hud::DatumKind;
use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, ACCENT_ORANGE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_ADD, ICON_CLEAR, ICON_CLOSE, ICON_DELETE, ICON_EDIT, ICON_GRID_4X4, ICON_LAYERS_OFF,
    ICON_SEARCH, ICON_VISIBILITY, ICON_VISIBILITY_OFF,
};

#[derive(Debug, Clone)]
pub struct PlaneItemInfo {
    pub index: usize,
    pub name: String,
    pub is_active: bool,
    pub is_custom: bool,
    pub custom_id: Option<u32>,
    /// Bidang kustom: tampil di viewport. Bidang standar selalu `true`.
    pub visible: bool,
}

/// Sumbu atau titik datum di drawer (Paritas 1.5).
#[derive(Debug, Clone)]
pub struct DatumItemInfo {
    /// `Axis` atau `Point` (bidang memakai [`PlaneItemInfo`]).
    pub kind: DatumKind,
    pub id: u32,
    pub name: String,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub enum PlanesDrawerEvent {
    SelectPlane(usize),
    DeletePlane(u32),
    CreateNewPlane,
    Close,
    /// Buka tool Datum dengan jenis tertentu.
    CreateDatum(DatumKind),
    /// Tampil/sembunyikan datum `(jenis, id)`; bidang memakai id kustomnya.
    ToggleVisible(DatumKind, u32),
    /// Ganti nama datum `(jenis, id, nama baru)`.
    Rename(DatumKind, u32, String),
    /// Hapus sumbu/titik datum (bidang memakai [`Self::DeletePlane`]).
    DeleteDatum(DatumKind, u32),
}

#[derive(Default)]
pub struct PlanesDrawer {
    pub search_query: String,
    pub custom_height: Option<f32>,
    /// Datum yang sedang diganti namanya: `(jenis, id, teks sunting)`.
    pub renaming: Option<(DatumKind, u32, String)>,
}

/// Satu baris kartu datum generik (dipakai bidang, sumbu, titik).
struct CardSpec<'a> {
    kind: DatumKind,
    /// `None` = bidang standar (tidak bisa disunting/dihapus/disembunyikan).
    id: Option<u32>,
    name: &'a str,
    is_active: bool,
    visible: bool,
    icon: &'static str,
    /// Klik nama → event ini (bidang: aktifkan).
    on_click: Option<PlanesDrawerEvent>,
    delete: Option<PlanesDrawerEvent>,
}

impl PlanesDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render panel daftar bidang referensi di pojok kanan bawah kanvas.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        planes: &[PlaneItemInfo],
        datums: &[DatumItemInfo],
        max_height: f32,
        _anchor_bottom_y: f32,
    ) -> Option<PlanesDrawerEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                const DRAWER_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH;
                ui.set_min_width(DRAWER_W);
                ui.set_max_width(DRAWER_W);
                ui.set_width(DRAWER_W);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                let query = self.search_query.trim().to_lowercase();
                let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
                let filtered: Vec<&PlaneItemInfo> =
                    planes.iter().filter(|item| matches(&item.name)).collect();
                let axes: Vec<&DatumItemInfo> = datums
                    .iter()
                    .filter(|d| d.kind == DatumKind::Axis && matches(&d.name))
                    .collect();
                let points: Vec<&DatumItemInfo> = datums
                    .iter()
                    .filter(|d| d.kind == DatumKind::Point && matches(&d.name))
                    .collect();
                let rows = filtered.len() + axes.len() + points.len() + 3;

                let estimated_h = (100.0 + (rows as f32 * 40.0)).clamp(140.0, max_height);
                let panel_h = self.custom_height.unwrap_or(estimated_h);

                // =========================================================================
                // 1. HEADER DRAWER (Judul, Badge Jumlah, Tombol + New, Tombol Close)
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                    // Ikon Layers Off
                    ui.label(
                        RichText::new(ICON_LAYERS_OFF.codepoint)
                            .size(14.0)
                            .color(ACCENT_BLUE),
                    );

                    // Judul Panel
                    ui.label(
                        RichText::new(t!("datum-drawer-title"))
                            .size(12.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );

                    // Badge Angka Lingkaran Total Bidang
                    let count_str = (planes.len() + datums.len()).to_string();
                    let font_id = egui::FontId::proportional(9.0);
                    let galley = ui.painter().layout_no_wrap(
                        count_str.clone(),
                        font_id,
                        Color32::from_rgb(180, 205, 235),
                    );
                    let diameter = (galley.size().x.max(galley.size().y) + 6.0).max(15.0);
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(diameter, diameter),
                        egui::Sense::hover(),
                    );
                    ui.painter().circle_filled(
                        rect.center(),
                        diameter * 0.5,
                        Color32::from_rgb(25, 45, 80),
                    );
                    ui.painter().circle_stroke(
                        rect.center(),
                        diameter * 0.5,
                        Stroke::new(0.5, Color32::from_rgb(60, 110, 190)),
                    );
                    ui.painter().galley(
                        rect.center() - galley.size() * 0.5,
                        galley,
                        Color32::from_rgb(180, 205, 235),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Tombol Close (X)
                        let close_btn = ui.add(
                            egui::Button::new(
                                RichText::new(ICON_CLOSE.codepoint)
                                    .size(12.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .frame(false)
                            .min_size(Vec2::new(16.0, 16.0)),
                        );
                        if close_btn
                            .on_hover_text(t!("param-close"))
                            .clicked()
                        {
                            event = Some(PlanesDrawerEvent::Close);
                        }

                        // Tombol + New Plane
                        let add_btn = ui.add(
                            egui::Button::new(
                                RichText::new(ICON_ADD.codepoint)
                                    .size(14.0)
                                    .color(ACCENT_BLUE),
                            )
                            .frame(false)
                            .min_size(Vec2::new(18.0, 18.0)),
                        );
                        if add_btn
                            .on_hover_text(t!("planes-drawer-new"))
                            .clicked()
                        {
                            event = Some(PlanesDrawerEvent::CreateNewPlane);
                        }
                    });
                });

                ui.add_space(1.0);

                // =========================================================================
                // 2. SEARCH BAR COMPACT
                // =========================================================================
                Frame::NONE
                    .fill(Color32::from_rgb(20, 23, 30))
                    .stroke(Stroke::new(0.5, BORDER_SUBTLE))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                            ui.label(
                                RichText::new(ICON_SEARCH.codepoint)
                                    .size(11.0)
                                    .color(TEXT_MUTED),
                            );

                            let edit_resp = ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text(
                                        RichText::new(t!("planes-drawer-search"))
                                            .size(10.0)
                                            .color(TEXT_MUTED),
                                    )
                                    .font(egui::FontId::proportional(10.5))
                                    .text_color(TEXT_PRIMARY)
                                    .frame(Frame::NONE)
                                    .desired_width(DRAWER_W - 60.0),
                            );

                            if !self.search_query.is_empty() {
                                let clear_btn = ui.add(
                                    egui::Button::new(
                                        RichText::new(ICON_CLEAR.codepoint)
                                            .size(10.0)
                                            .color(TEXT_SECONDARY),
                                    )
                                    .frame(false)
                                    .min_size(Vec2::new(12.0, 12.0)),
                                );
                                if clear_btn.clicked() {
                                    self.search_query.clear();
                                    edit_resp.request_focus();
                                }
                            }
                        });
                    });

                ui.add_space(2.0);

                // =========================================================================
                // 3. DAFTAR DATUM (SCROLL AREA): Bidang, Sumbu, Titik
                // =========================================================================
                ScrollArea::vertical()
                    .id_salt("planes_drawer_scroll")
                    .max_height(panel_h - 60.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

                        // --- Bidang ---
                        if let Some(ev) = Self::section_header(ui, DatumKind::Plane, filtered.len()) {
                            event = Some(ev);
                        }
                        if filtered.is_empty() {
                            Self::empty_row(ui, &t!("planes-drawer-empty"));
                        }
                        for plane in filtered {
                            let spec = CardSpec {
                                kind: DatumKind::Plane,
                                id: plane.custom_id,
                                name: &plane.name,
                                is_active: plane.is_active,
                                visible: plane.visible,
                                icon: if plane.is_custom {
                                    ICON_GRID_4X4.codepoint
                                } else {
                                    ICON_LAYERS_OFF.codepoint
                                },
                                on_click: Some(PlanesDrawerEvent::SelectPlane(plane.index)),
                                delete: plane.custom_id.map(PlanesDrawerEvent::DeletePlane),
                            };
                            if let Some(ev) = self.card(ui, &format!("plane_item_{}", plane.index), spec) {
                                event = Some(ev);
                            }
                        }

                        // --- Sumbu & Titik ---
                        for (kind, list) in [(DatumKind::Axis, &axes), (DatumKind::Point, &points)] {
                            ui.add_space(4.0);
                            if let Some(ev) = Self::section_header(ui, kind, list.len()) {
                                event = Some(ev);
                            }
                            if list.is_empty() {
                                let empty = match kind {
                                    DatumKind::Axis => t!("datum-drawer-no-axes"),
                                    _ => t!("datum-drawer-no-points"),
                                };
                                Self::empty_row(ui, &empty);
                            }
                            for item in list.iter() {
                                let spec = CardSpec {
                                    kind,
                                    id: Some(item.id),
                                    name: &item.name,
                                    is_active: false,
                                    visible: item.visible,
                                    icon: kind.icon(),
                                    on_click: None,
                                    delete: Some(PlanesDrawerEvent::DeleteDatum(kind, item.id)),
                                };
                                let salt = format!("datum_item_{kind:?}_{}", item.id);
                                if let Some(ev) = self.card(ui, &salt, spec) {
                                    event = Some(ev);
                                }
                            }
                        }
                    });
            });
        });

        event
    }

    /// Judul seksi (Bidang/Sumbu/Titik) + tombol tambah.
    fn section_header(ui: &mut Ui, kind: DatumKind, count: usize) -> Option<PlanesDrawerEvent> {
        let mut event = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(5.0, 0.0);
            ui.label(RichText::new(kind.icon()).size(11.0).color(ACCENT_BLUE));
            let title = match kind {
                DatumKind::Plane => t!("datum-drawer-planes"),
                DatumKind::Axis => t!("datum-drawer-axes"),
                DatumKind::Point => t!("datum-drawer-points"),
            };
            ui.label(
                RichText::new(format!("{title} ({count})"))
                    .size(10.0)
                    .strong()
                    .color(TEXT_SECONDARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let tip = match kind {
                    DatumKind::Plane => t!("planes-drawer-new"),
                    DatumKind::Axis => t!("datum-drawer-new-axis"),
                    DatumKind::Point => t!("datum-drawer-new-point"),
                };
                let add = ui.add(
                    egui::Button::new(RichText::new(ICON_ADD.codepoint).size(12.0).color(ACCENT_BLUE))
                        .frame(false)
                        .min_size(Vec2::new(16.0, 16.0)),
                );
                if add.on_hover_text(tip).clicked() {
                    event = Some(match kind {
                        DatumKind::Plane => PlanesDrawerEvent::CreateNewPlane,
                        k => PlanesDrawerEvent::CreateDatum(k),
                    });
                }
            });
        });
        event
    }

    fn empty_row(ui: &mut Ui, text: &str) {
        ui.label(RichText::new(text).size(10.0).color(TEXT_MUTED));
    }

    /// Kartu satu datum: ikon, nama (klik = aktifkan / klik ganda = ganti
    /// nama), tombol tampil/sembunyi, edit nama, hapus, badge aktif.
    fn card(&mut self, ui: &mut Ui, salt: &str, spec: CardSpec<'_>) -> Option<PlanesDrawerEvent> {
        let mut event = None;
        ui.push_id(salt, |ui| {
            let is_active = spec.is_active;
            let card_bg = if is_active {
                Color32::from_rgb(18, 38, 68)
            } else {
                Color32::from_rgb(26, 29, 36)
            };
            let card_stroke = if is_active {
                Stroke::new(1.0, ACCENT_BLUE)
            } else {
                Stroke::new(0.5, BORDER_SUBTLE)
            };
            card_frame()
                .fill(card_bg)
                .stroke(card_stroke)
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(7, 4))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        let icon_color = if is_active {
                            ACCENT_BLUE
                        } else if spec.id.is_some() {
                            ACCENT_ORANGE
                        } else {
                            TEXT_SECONDARY
                        };
                        ui.label(RichText::new(spec.icon).size(12.5).color(icon_color));

                        let renaming_this = matches!(
                            &self.renaming,
                            Some((k, id, _)) if *k == spec.kind && Some(*id) == spec.id
                        );
                        if renaming_this {
                            if let Some((_, id, buf)) = self.renaming.as_mut() {
                                let id = *id;
                                let resp = ui.add(
                                    egui::TextEdit::singleline(buf)
                                        .font(egui::FontId::proportional(11.0))
                                        .desired_width(150.0),
                                );
                                if !resp.has_focus() && !resp.lost_focus() {
                                    resp.request_focus();
                                }
                                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                                if esc {
                                    self.renaming = None;
                                } else if enter || resp.lost_focus() {
                                    let name = buf.trim().to_string();
                                    if !name.is_empty() {
                                        event = Some(PlanesDrawerEvent::Rename(spec.kind, id, name));
                                    }
                                    self.renaming = None;
                                }
                            }
                            return;
                        }

                        let text_color = if is_active {
                            Color32::WHITE
                        } else if spec.visible {
                            TEXT_PRIMARY
                        } else {
                            TEXT_MUTED
                        };
                        let name_resp = ui.add(
                            egui::Label::new(
                                RichText::new(spec.name).size(11.0).strong().color(text_color),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if name_resp.hovered() && spec.on_click.is_some() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if name_resp.double_clicked() {
                            if let Some(id) = spec.id {
                                self.renaming = Some((spec.kind, id, spec.name.to_string()));
                            }
                        } else if name_resp.clicked() {
                            if let Some(ev) = spec.on_click.clone() {
                                event = Some(ev);
                            }
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let Some(id) = spec.id else {
                                Self::active_badge(ui, is_active);
                                return;
                            };
                            if let Some(del) = spec.delete.clone() {
                                let del_btn = ui.add(
                                    egui::Button::new(
                                        RichText::new(ICON_DELETE.codepoint)
                                            .size(13.0)
                                            .color(Color32::from_rgb(240, 90, 90)),
                                    )
                                    .frame(false)
                                    .min_size(Vec2::new(18.0, 18.0)),
                                );
                                if del_btn.on_hover_text(t!("planes-drawer-delete-tooltip")).clicked() {
                                    event = Some(del);
                                }
                            }
                            let edit = ui.add(
                                egui::Button::new(
                                    RichText::new(ICON_EDIT.codepoint).size(12.0).color(TEXT_SECONDARY),
                                )
                                .frame(false)
                                .min_size(Vec2::new(18.0, 18.0)),
                            );
                            if edit.on_hover_text(t!("datum-drawer-rename")).clicked() {
                                self.renaming = Some((spec.kind, id, spec.name.to_string()));
                            }
                            let eye = if spec.visible {
                                ICON_VISIBILITY.codepoint
                            } else {
                                ICON_VISIBILITY_OFF.codepoint
                            };
                            let vis = ui.add(
                                egui::Button::new(RichText::new(eye).size(12.0).color(TEXT_SECONDARY))
                                    .frame(false)
                                    .min_size(Vec2::new(18.0, 18.0)),
                            );
                            if vis.on_hover_text(t!("datum-drawer-toggle-visible")).clicked() {
                                event = Some(PlanesDrawerEvent::ToggleVisible(spec.kind, id));
                            }
                            Self::active_badge(ui, is_active);
                        });
                    });
                });
        });
        event
    }

    fn active_badge(ui: &mut Ui, is_active: bool) {
        if !is_active {
            return;
        }
        Frame::NONE
            .fill(Color32::from_rgb(14, 80, 160))
            .corner_radius(CornerRadius::same(3))
            .inner_margin(Margin::symmetric(4, 1))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(t!("planes-drawer-active"))
                        .size(9.0)
                        .strong()
                        .color(Color32::WHITE),
                );
            });
    }
}
