//! Outliner Drawer (Items Tree) bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan drawer mengambang di sisi kiri untuk navigasi hierarki dokumen:
//! sketsa aktif, bidang konstruksi, dan daftar solid body 3D dengan kontrol
//! visibilitas (ikon Material Visibility) dan status seleksi.

use egui::{RichText, ScrollArea, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_CATEGORY, ICON_EDIT, ICON_FOLDER, ICON_LAYERS, ICON_VISIBILITY, ICON_VISIBILITY_OFF,
};
use crate::theme::{card_frame, glass_frame, ACCENT_BLUE, TEXT_PRIMARY, TEXT_SECONDARY};

pub struct BodyItemInfo {
    pub id_raw: u64,
    pub name: String,
    pub visible: bool,
    pub selected: bool,
}

pub struct SketchPlaneItemInfo {
    pub index: usize,
    pub name: String,
    pub active: bool,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub enum ItemsDrawerEvent {
    ToggleBodyVisibility(u64),
    SelectBody { id_raw: u64, extend: bool },
    ToggleSketchVisibility(usize),
    SelectSketchPlane(usize),
}

pub struct ItemsDrawer {
    search_query: String,
}

impl Default for ItemsDrawer {
    fn default() -> Self {
        Self {
            search_query: String::new(),
        }
    }
}

impl ItemsDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render drawer outliner items. Mengembalikan `Option<ItemsDrawerEvent>`.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        sketch_planes: &[SketchPlaneItemInfo],
        bodies: &[BodyItemInfo],
    ) -> Option<ItemsDrawerEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_width(220.0);
            ui.set_max_height(480.0);
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);

            // Header
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} All Items", ICON_FOLDER)).strong().size(13.0).color(TEXT_PRIMARY));
            });

            // Filter Pencarian
            ui.text_edit_singleline(&mut self.search_query);

            ui.separator();

            let query = self.search_query.to_lowercase();

            ScrollArea::vertical().show(ui, |ui| {
                // Section: Sketch Planes
                ui.label(RichText::new("SKETCHES").size(10.0).color(TEXT_SECONDARY).strong());
                for sp in sketch_planes {
                    if !query.is_empty() && !sp.name.to_lowercase().contains(&query) {
                        continue;
                    }

                    card_frame().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Ikon mata visibilitas
                            let eye = if sp.visible { ICON_VISIBILITY } else { ICON_VISIBILITY_OFF };
                            if ui.small_button(eye).clicked() {
                                event = Some(ItemsDrawerEvent::ToggleSketchVisibility(sp.index));
                            }

                            // Ikon & Nama Sketch
                            let label_text = if sp.active {
                                RichText::new(format!("{} {} (Aktif)", ICON_EDIT, sp.name)).strong().color(ACCENT_BLUE)
                            } else {
                                RichText::new(format!("{} {}", ICON_EDIT, sp.name)).color(TEXT_PRIMARY)
                            };

                            if ui.selectable_label(sp.active, label_text).clicked() {
                                event = Some(ItemsDrawerEvent::SelectSketchPlane(sp.index));
                            }
                        });
                    });
                }

                ui.add_space(6.0);

                // Section: Planes (Konstruksi)
                ui.label(RichText::new("PLANES").size(10.0).color(TEXT_SECONDARY).strong());
                card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(ICON_VISIBILITY).color(TEXT_SECONDARY));
                        ui.label(RichText::new(format!("{} Plane 01 (XY)", ICON_LAYERS)).color(TEXT_PRIMARY));
                    });
                });

                ui.add_space(6.0);

                // Section: 3D Bodies
                ui.label(RichText::new(format!("BODIES ({})", bodies.len())).size(10.0).color(TEXT_SECONDARY).strong());
                if bodies.is_empty() {
                    ui.label(RichText::new("Belum ada body 3D").size(11.0).color(TEXT_SECONDARY));
                } else {
                    for b in bodies {
                        if !query.is_empty() && !b.name.to_lowercase().contains(&query) {
                            continue;
                        }

                        card_frame().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let eye = if b.visible { ICON_VISIBILITY } else { ICON_VISIBILITY_OFF };
                                if ui.small_button(eye).clicked() {
                                    event = Some(ItemsDrawerEvent::ToggleBodyVisibility(b.id_raw));
                                }

                                let text = if b.selected {
                                    RichText::new(format!("{} {}", ICON_CATEGORY, b.name)).strong().color(ACCENT_BLUE)
                                } else {
                                    RichText::new(format!("{} {}", ICON_CATEGORY, b.name)).color(TEXT_PRIMARY)
                                };

                                let resp = ui.selectable_label(b.selected, text);
                                if resp.clicked() {
                                    let extend = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                                    event = Some(ItemsDrawerEvent::SelectBody {
                                        id_raw: b.id_raw,
                                        extend,
                                    });
                                }
                            });
                        });
                    }
                }
            });
        });

        event
    }
}
