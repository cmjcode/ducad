//! Panel "Properti Massa" di GUI (P16): data dihitung dari
//! `ducad_engine::inspect` untuk body terpilih (atau body pertama) dan
//! di-cache sampai geometri/material berubah; material mekanik diganti
//! lewat command undo-able; penanda pusat massa digambar di viewport.

use std::hash::{Hash, Hasher};

use ducad_core::{BodyId, MaterialSource};
use ducad_ui::{MassPanelData, MassPanelEvent, MassPropertiesPanel};

use crate::app::DuCADApp;
use crate::viewport::world_to_screen_pos;

/// Keadaan panel properti massa milik `DuCADApp`.
#[derive(Default)]
pub struct MassState {
    pub panel_open: bool,
    pub panel: MassPropertiesPanel,
    /// (tanda tangan, data) hasil hitung terakhir.
    cache: Option<(u64, MassPanelData)>,
}

impl DuCADApp {
    /// Body yang ditampilkan panel: yang terpilih, selain itu body pertama.
    fn mass_target(&self) -> Option<BodyId> {
        self.selected_bodies
            .iter()
            .copied()
            .find(|id| self.model.doc.bodies.contains_key(*id))
            .or_else(|| self.model.doc.bodies.keys().next())
    }

    /// Tanda tangan murah: target + alamat mesh semua body + material.
    fn mass_signature(&self, target: Option<BodyId>) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{target:?}").hash(&mut h);
        for (id, b) in self.model.doc.bodies.iter() {
            format!("{id:?}").hash(&mut h);
            b.name.hash(&mut h);
            format!("{:?}{:?}", b.mechanical, b.material.preset).hash(&mut h);
            if let Some(g) = self.model.geometry.get(id) {
                (std::sync::Arc::as_ptr(&g.mesh) as usize).hash(&mut h);
            }
        }
        h.finish()
    }

    /// Data panel untuk keadaan model saat ini (dihitung ulang hanya bila
    /// tanda tangan berubah — `BRepGProp` terlalu mahal untuk tiap frame).
    pub fn mass_panel_data(&mut self) -> MassPanelData {
        let target = self.mass_target();
        let sig = self.mass_signature(target);
        if let Some((cached, data)) = &self.mass.cache {
            if *cached == sig {
                return data.clone();
            }
        }
        let data = self.compute_mass_panel_data(target);
        self.mass.cache = Some((sig, data.clone()));
        data
    }

    fn compute_mass_panel_data(&self, target: Option<BodyId>) -> MassPanelData {
        let doc = &self.model.doc;
        let Some((body, geo)) = target.and_then(|id| {
            Some((doc.bodies.get(id)?, self.model.geometry.get(id)?))
        }) else {
            return MassPanelData::default();
        };
        let mp = geo.shape.mass_properties();
        let solid = mp.volume_mm3 > 1e-9;
        let density = doc.density_of(body);
        let rho = density.filter(|_| solid).map(|d| d / 1000.0);
        let (principal, axes) = mp.principal();
        let scale = |t: [[f64; 3]; 3], k: f64| t.map(|row| row.map(|v| v * k));
        let assembly = (doc.bodies.len() >= 2)
            .then(|| ducad_engine::inspect::assembly_mass(&self.model))
            .flatten()
            .map(|a| (a.total_mass_g, a.center_of_mass));
        MassPanelData {
            body_name: Some(body.name.clone()),
            material_key: body.mechanical.as_ref().and_then(|m| match m {
                MaterialSource::Library(key) => Some(key.clone()),
                MaterialSource::Custom(_) => Some("custom".to_string()),
                MaterialSource::Preset(_) => None,
            }),
            mechanical: doc.mechanical_of(body),
            density_g_cm3: density,
            mass_g: density.map(|d| mp.volume_mm3 / 1000.0 * d),
            volume_mm3: mp.volume_mm3,
            area_mm2: geo.shape.surface_area(),
            center_of_mass: solid.then_some(mp.centroid),
            inertia_com: rho.map(|r| scale(mp.inertia_com(), r)),
            inertia_origin: rho.map(|r| scale(mp.inertia_origin, r)),
            principal_moments: rho.map(|r| principal.map(|v| v * r)),
            principal_axes: solid.then_some(axes),
            radius_of_gyration: solid
                .then(|| principal.map(|v| (v.max(0.0) / mp.volume_mm3).sqrt())),
            assembly,
        }
    }

    /// Terapkan material mekanik ke body terpilih (atau target panel).
    fn set_mechanical_material(&mut self, source: MaterialSource) {
        let mut targets: Vec<BodyId> = self.selected_bodies.iter().copied().collect();
        if targets.is_empty() {
            targets.extend(self.mass_target());
        }
        for id in targets {
            let cmd = crate::model::SetBodyMechanicalCommand::new(
                "Material",
                id,
                Some(source.clone()),
            );
            self.model_undo.execute(Box::new(cmd), &mut self.model);
        }
        self.model_status = Some(ducad_i18n::t!("mass-material-applied"));
    }

    pub fn handle_mass_panel_event(&mut self, event: MassPanelEvent) {
        match event {
            MassPanelEvent::Close => self.mass.panel_open = false,
            MassPanelEvent::ToggleMarker => {
                self.mass.panel.show_marker = !self.mass.panel.show_marker;
            }
            MassPanelEvent::SetLibraryMaterial(key) => {
                self.set_mechanical_material(MaterialSource::Library(key));
            }
            MassPanelEvent::SetCustomMaterial(props) => {
                self.set_mechanical_material(MaterialSource::Custom(props));
            }
        }
    }

    /// Penanda pusat massa: bola kecil + tiga sumbu utama (merah = momen
    /// terkecil, hijau = menengah, biru = terbesar).
    pub fn paint_mass_marker(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if !(self.mass.panel_open && self.mass.panel.show_marker) {
            return;
        }
        let data = self.mass_panel_data();
        let (Some(com), Some(axes)) = (data.center_of_mass, data.principal_axes) else {
            return;
        };
        let to_vec = |p: [f64; 3]| glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
        let center = to_vec(com);
        let Some(center_px) = world_to_screen_pos(&self.camera, rect, center) else {
            return;
        };
        // Panjang sumbu mengikuti ukuran body (radius girasi terbesar).
        let reach = data
            .radius_of_gyration
            .map(|r| r[0].max(r[1]).max(r[2]) as f32)
            .unwrap_or(10.0)
            .max(1.0);
        let painter = ui.painter_at(rect);
        let colors = [
            egui::Color32::from_rgb(255, 69, 58),
            egui::Color32::from_rgb(48, 209, 88),
            egui::Color32::from_rgb(10, 132, 255),
        ];
        for (axis, color) in axes.iter().zip(colors) {
            let dir = to_vec(*axis) * reach;
            let ends = (
                world_to_screen_pos(&self.camera, rect, center - dir),
                world_to_screen_pos(&self.camera, rect, center + dir),
            );
            if let (Some(a), Some(b)) = ends {
                painter.line_segment([a, b], egui::Stroke::new(1.5, color));
            }
        }
        painter.circle_filled(center_px, 5.0, egui::Color32::from_rgb(255, 214, 10));
        painter.circle_stroke(center_px, 5.0, egui::Stroke::new(1.0, egui::Color32::BLACK));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_box() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        let shape = ducad_kernel::make_box(10.0, 20.0, 30.0, false).unwrap();
        let id = app.model.doc.add_body("box");
        app.model
            .geometry
            .insert(id, crate::model::BodyGeometry::from_shape(shape));
        app
    }

    #[test]
    fn panel_data_matches_engine_and_follows_material() {
        let mut app = app_with_box();
        let d = app.mass_panel_data();
        assert_eq!(d.body_name.as_deref(), Some("box"));
        assert!((d.volume_mm3 - 6000.0).abs() < 1e-6);
        // Preset visual bawaan: 1,20 g/cm³.
        assert!((d.mass_g.unwrap() - 7.2).abs() < 1e-9);
        assert_eq!(d.center_of_mass.unwrap().map(|v| v.round()), [5.0, 10.0, 15.0]);
        assert_eq!(d.material_key, None);

        app.handle_mass_panel_event(MassPanelEvent::SetLibraryMaterial("s235".into()));
        let d = app.mass_panel_data();
        assert_eq!(d.material_key.as_deref(), Some("s235"));
        assert!((d.mass_g.unwrap() - 47.1).abs() < 1e-9);
        // Izz = m(a²+b²)/12.
        let izz = 47.1 * (100.0 + 400.0) / 12.0;
        assert!((d.inertia_com.unwrap()[2][2] - izz).abs() / izz < 1e-6);

        app.model_undo.undo(&mut app.model);
        assert_eq!(app.mass_panel_data().material_key, None);
    }

    #[test]
    fn panel_events_toggle_state() {
        let mut app = app_with_box();
        app.mass.panel_open = true;
        app.handle_mass_panel_event(MassPanelEvent::ToggleMarker);
        assert!(app.mass.panel.show_marker);
        app.handle_mass_panel_event(MassPanelEvent::Close);
        assert!(!app.mass.panel_open);
        assert_eq!(DuCADApp::new_for_test().mass_panel_data(), MassPanelData::default());
    }
}
