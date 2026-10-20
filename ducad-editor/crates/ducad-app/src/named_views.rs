//! Tampilan kamera bernama (Paritas 3.13): simpan pose kamera orbit
//! (target, yaw, pitch, jarak) dengan nama, terapkan, ganti nama, hapus.
//! Disimpan di dokumen `.ducad` (blok `datums.views`).

use ducad_io::native::NativeNamedView;

use crate::app::DuCADApp;

/// Status jendela tampilan bernama.
#[derive(Default)]
pub struct NamedViewsState {
    pub open: bool,
    /// Nama untuk tampilan baru.
    pub new_name: String,
    /// Indeks yang sedang diganti namanya + teks sementaranya.
    pub renaming: Option<(usize, String)>,
}

impl DuCADApp {
    /// Simpan pose kamera saat ini dengan `name` (kosong → "Tampilan N";
    /// nama yang sudah ada ditimpa).
    pub fn save_named_view(&mut self, name: &str) {
        let name = name.trim();
        let name = if name.is_empty() {
            ducad_i18n::t!("views-default-name", n = self.datums.views.len() + 1)
        } else {
            name.to_string()
        };
        let v = NativeNamedView {
            name: name.clone(),
            target: self.camera.target.to_array(),
            yaw: self.camera.yaw,
            pitch: self.camera.pitch,
            distance: self.camera.distance,
        };
        if !v.is_valid() {
            return;
        }
        match self.datums.views.iter_mut().find(|x| x.name == name) {
            Some(slot) => *slot = v,
            None => self.datums.views.push(v),
        }
        self.model.doc.dirty = true;
        self.model_status = Some(ducad_i18n::t!("views-saved", name = name.as_str()));
    }

    /// Terapkan tampilan ke-`i` (mode orbit).
    pub fn apply_named_view(&mut self, i: usize) {
        let Some(v) = self.datums.views.get(i).cloned() else {
            return;
        };
        if !v.is_valid() {
            return;
        }
        self.camera.set_mode(ducad_render::camera::CameraMode::Orbit);
        self.camera.target = glam::Vec3::from_array(v.target);
        self.camera.yaw = v.yaw;
        self.camera.pitch = v.pitch;
        self.camera.distance = v.distance;
    }

    /// Ganti nama tampilan ke-`i`; nama kosong/duplikat ditolak.
    pub fn rename_named_view(&mut self, i: usize, name: &str) -> bool {
        let name = name.trim();
        let taken = self
            .datums
            .views
            .iter()
            .enumerate()
            .any(|(k, v)| k != i && v.name == name);
        if name.is_empty() || taken {
            return false;
        }
        match self.datums.views.get_mut(i) {
            Some(v) => {
                v.name = name.to_string();
                self.model.doc.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn delete_named_view(&mut self, i: usize) {
        if i < self.datums.views.len() {
            self.datums.views.remove(i);
            self.model.doc.dirty = true;
        }
    }

    /// Jendela tampilan bernama.
    pub fn show_named_views(&mut self, ctx: &egui::Context) {
        if !self.named_views.open {
            return;
        }
        let mut open = true;
        let mut save = false;
        let mut apply = None;
        let mut delete = None;
        let mut commit_rename = None;
        let views: Vec<String> = self.datums.views.iter().map(|v| v.name.clone()).collect();
        let st = &mut self.named_views;
        egui::Window::new(ducad_i18n::t!("views-title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                if views.is_empty() {
                    ui.label(egui::RichText::new(ducad_i18n::t!("views-empty")).small());
                }
                for (i, name) in views.iter().enumerate() {
                    ui.horizontal(|ui| {
                        match &mut st.renaming {
                            Some((k, text)) if *k == i => {
                                let r = ui.text_edit_singleline(text);
                                if r.lost_focus() {
                                    commit_rename = Some((i, text.clone()));
                                }
                            }
                            _ => {
                                if ui
                                    .button(name)
                                    .on_hover_text(ducad_i18n::t!("views-apply"))
                                    .clicked()
                                {
                                    apply = Some(i);
                                }
                            }
                        }
                        if ui
                            .small_button(egui_icons::icons::ICON_EDIT.codepoint)
                            .on_hover_text(ducad_i18n::t!("views-rename"))
                            .clicked()
                        {
                            st.renaming = Some((i, name.clone()));
                        }
                        if ui
                            .small_button(egui_icons::icons::ICON_DELETE.codepoint)
                            .on_hover_text(ducad_i18n::t!("views-delete"))
                            .clicked()
                        {
                            delete = Some(i);
                        }
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut st.new_name)
                            .hint_text(ducad_i18n::t!("views-name-hint"))
                            .desired_width(150.0),
                    );
                    save = ui.button(ducad_i18n::t!("views-save")).clicked();
                });
            });
        if save {
            let name = std::mem::take(&mut self.named_views.new_name);
            self.save_named_view(&name);
        }
        if let Some(i) = apply {
            self.apply_named_view(i);
        }
        if let Some((i, text)) = commit_rename {
            self.named_views.renaming = None;
            if !self.rename_named_view(i, &text) {
                self.model_status = Some(ducad_i18n::t!("views-rename-invalid"));
            }
        }
        if let Some(i) = delete {
            self.named_views.renaming = None;
            self.delete_named_view(i);
        }
        if !open {
            self.named_views.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_apply_rename_delete_and_persist() {
        let mut app = DuCADApp::new_for_test();
        app.camera.yaw = 1.0;
        app.camera.pitch = 0.2;
        app.camera.distance = 77.0;
        app.save_named_view("Depan");
        app.save_named_view("");
        assert_eq!(app.datums.views.len(), 2);
        app.camera.yaw = 0.0;
        app.camera.distance = 10.0;
        app.apply_named_view(0);
        assert_eq!(app.camera.yaw, 1.0);
        assert_eq!(app.camera.distance, 77.0);
        assert!(!app.rename_named_view(1, "Depan"), "duplikat ditolak");
        assert!(app.rename_named_view(1, "Atas"));
        let saved = app.native_datums();
        assert_eq!(saved.views.len(), 2);
        app.delete_named_view(0);
        assert_eq!(app.datums.views[0].name, "Atas");
        app.load_native_datums(saved);
        assert_eq!(app.datums.views.len(), 2);
        app.apply_named_view(99);
    }
}
