//! Edit Teks (Paritas 0.8). Teks dari tool Text disimpan sebagai entitas
//! kurva bernama grup `Teks: <isi>`. Double-click pada salah satu glyph (atau
//! perintah palette) membuka popup; isi baru dirender ulang di posisi kiri-atas
//! yang sama dan mengganti glyph lama dalam SATU langkah undo.

use ducad_sketch::{translate_entity, Entity, EntityId, ReplaceEntities};
use glam::DVec2;

use crate::app::DuCADApp;

/// Awalan nama grup entitas teks.
pub const TEXT_GROUP_PREFIX: &str = "Teks: ";

pub fn text_group_name(content: &str) -> String {
    format!("{TEXT_GROUP_PREFIX}{}", content.trim())
}

#[derive(Default)]
pub struct TextEditState {
    /// Nama grup teks yang sedang diedit.
    pub group: Option<String>,
    pub input: String,
    pub height_mm: f64,
}

fn bbox_of<'a>(entities: impl Iterator<Item = &'a Entity>) -> Option<(DVec2, DVec2)> {
    entities.filter_map(Entity::bounding_box).reduce(|(a0, a1), (b0, b1)| (a0.min(b0), a1.max(b1)))
}

impl DuCADApp {
    /// Nama grup teks milik entitas `id`, bila ada.
    pub fn text_group_of(&self, id: EntityId) -> Option<String> {
        self.sketch()
            .entity_names
            .get(&id)
            .filter(|n| n.starts_with(TEXT_GROUP_PREFIX))
            .cloned()
    }

    fn text_group_ids(&self, group: &str) -> Vec<EntityId> {
        self.sketch()
            .entity_names
            .iter()
            .filter(|(id, n)| n.as_str() == group && self.sketch().entities.contains_key(**id))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Buka popup edit bila `id` bagian dari teks. `true` bila terbuka.
    pub fn open_text_edit_for(&mut self, id: EntityId) -> bool {
        let Some(group) = self.text_group_of(id) else { return false };
        let ids = self.text_group_ids(&group);
        self.selected = ids.iter().copied().collect();
        self.text_edit.input = group.trim_start_matches(TEXT_GROUP_PREFIX).to_string();
        self.text_edit.height_mm = self.text_popup_state.font_height_mm;
        self.text_edit.group = Some(group);
        true
    }

    /// Buka popup edit untuk seleksi saat ini (perintah palette).
    pub fn open_text_edit_from_selection(&mut self) {
        let first = self.selected.iter().copied().find(|id| self.text_group_of(*id).is_some());
        if !first.is_some_and(|id| self.open_text_edit_for(id)) {
            self.model_status = Some(ducad_i18n::t!("text-edit-need-text"));
        }
    }

    /// Ganti isi teks grup `group` dengan `content` setinggi `height_mm`.
    pub fn commit_text_edit(&mut self, group: &str, content: &str, height_mm: f64) {
        if content.trim().is_empty() || !height_mm.is_finite() || height_mm <= 0.0 {
            self.model_status = Some(ducad_i18n::t!("text-edit-invalid"));
            return;
        }
        let old_ids = self.text_group_ids(group);
        let Some((old_min, old_max)) = bbox_of(old_ids.iter().filter_map(|id| self.sketch().entities.get(*id))) else {
            return;
        };
        let options = ducad_sketch::TextOptions {
            font_height_mm: height_mm,
            letter_spacing: self.text_popup_state.letter_spacing,
            line_spacing: self.text_popup_state.line_spacing,
            align: self.text_popup_state.align,
            font_preset: self.text_popup_state.font_preset,
            is_construction: self.construction_mode,
        };
        let fresh = match ducad_sketch::text_to_entities(content, DVec2::ZERO, &options, self.custom_font_bytes.as_deref()) {
            Ok(e) if !e.is_empty() => e,
            Ok(_) => {
                self.model_status = Some(ducad_i18n::t!("text-edit-invalid"));
                return;
            }
            Err(e) => {
                self.model_status = Some(e);
                return;
            }
        };
        let Some((new_min, new_max)) = bbox_of(fresh.iter()) else { return };
        // Jangkar kiri-atas: tinggi huruf kapital lebih stabil daripada descender.
        let delta = DVec2::new(old_min.x - new_min.x, old_max.y - new_max.y);
        let moved: Vec<Entity> = fresh.iter().map(|e| translate_entity(e, delta)).collect();
        self.execute_sketch_command(Box::new(
            ReplaceEntities::new("Edit Teks", old_ids, moved).with_name(text_group_name(content)),
        ));
        self.selected.clear();
        self.model_status = Some(ducad_i18n::t!("text-edit-done"));
    }

    /// Popup edit teks.
    pub fn show_text_edit(&mut self, ctx: &egui::Context) {
        let Some(group) = self.text_edit.group.clone() else { return };
        let mut open = true;
        let mut apply = false;
        egui::Window::new(ducad_i18n::t!("text-edit-title"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .show(ctx, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.text_edit.input).desired_rows(2));
                ui.horizontal(|ui| {
                    ui.label(ducad_i18n::t!("text-edit-height"));
                    ui.add(
                        egui::DragValue::new(&mut self.text_edit.height_mm)
                            .speed(0.1)
                            .range(0.1..=1000.0)
                            .suffix(" mm"),
                    );
                });
                apply = ui.button(ducad_i18n::t!("text-edit-apply")).clicked();
            });
        if apply {
            let (content, h) = (self.text_edit.input.clone(), self.text_edit.height_mm);
            self.commit_text_edit(&group, &content, h);
            self.text_edit.group = None;
        } else if !open {
            self.text_edit.group = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_text(content: &str) -> (DuCADApp, String) {
        let mut app = DuCADApp::new_for_test();
        let options = ducad_sketch::TextOptions {
            font_height_mm: 10.0,
            letter_spacing: 1.0,
            line_spacing: 1.2,
            align: Default::default(),
            font_preset: Default::default(),
            is_construction: false,
        };
        let ents = ducad_sketch::text_to_entities(content, DVec2::new(50.0, 20.0), &options, None).unwrap();
        let name = text_group_name(content);
        app.execute_sketch_command(Box::new(ducad_sketch::InsertEntities::with_group("Teks 2D", ents, name.clone())));
        (app, name)
    }

    #[test]
    fn edit_replaces_glyphs_keeps_left_edge_and_undoes() {
        let (mut app, group) = app_with_text("AB");
        let before = app.text_group_ids(&group).len();
        let left_before = bbox_of(app.sketch().entities.values()).unwrap().0.x;
        let any = app.text_group_ids(&group)[0];
        assert!(app.open_text_edit_for(any));
        assert_eq!(app.text_edit.input, "AB");
        app.commit_text_edit(&group, "ABCD", 10.0);
        let new_group = text_group_name("ABCD");
        assert!(app.text_group_ids(&group).is_empty());
        assert!(app.text_group_ids(&new_group).len() > before);
        let left_after = bbox_of(app.sketch().entities.values()).unwrap().0.x;
        assert!((left_after - left_before).abs() < 1e-6);
        app.undo();
        assert_eq!(app.text_group_ids(&group).len(), before);
        assert!(app.text_group_ids(&new_group).is_empty());
    }

    #[test]
    fn edit_rejects_empty_text_without_change() {
        let (mut app, group) = app_with_text("X");
        let n = app.sketch().entities.len();
        app.commit_text_edit(&group, "   ", 10.0);
        app.commit_text_edit(&group, "Y", f64::NAN);
        assert_eq!(app.sketch().entities.len(), n);
        assert_eq!(app.text_group_ids(&group).len(), n);
    }

    #[test]
    fn non_text_entity_does_not_open_editor() {
        let mut app = DuCADApp::new_for_test();
        let id = app.sketch_mut().entities.insert(Entity::line(DVec2::ZERO, DVec2::X));
        assert!(!app.open_text_edit_for(id));
        assert!(app.text_edit.group.is_none());
    }
}
