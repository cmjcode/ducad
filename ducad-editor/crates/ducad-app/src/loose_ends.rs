//! Loose Ends (Paritas 0.16): sorot ujung entitas yang menggantung — sebab
//! paling umum sketsa gagal membentuk profil tertutup. Muncul otomatis saat
//! extrude gagal, dan bisa dinyalakan/dimatikan lewat command palette.

use ducad_sketch::gaps::{dangling_endpoints, JOIN_EPS};
use glam::DVec2;

use crate::app::DuCADApp;
use crate::viewport::world_to_screen_pos;

#[derive(Default)]
pub struct LooseEndsState {
    /// Titik ujung terbuka yang sedang disorot (koordinat bidang sketsa).
    pub points: Option<Vec<DVec2>>,
}

impl DuCADApp {
    /// Hitung ujung terbuka sketsa aktif.
    pub fn compute_loose_ends(&self) -> Vec<DVec2> {
        dangling_endpoints(self.sketch(), JOIN_EPS)
    }

    /// Sorot ujung terbuka bila ada; kembalikan jumlahnya.
    pub fn highlight_loose_ends(&mut self) -> usize {
        let pts = self.compute_loose_ends();
        let n = pts.len();
        self.loose_ends.points = (n > 0).then_some(pts);
        n
    }

    /// Perintah palette: nyalakan/matikan sorotan.
    pub fn toggle_loose_ends(&mut self) {
        if self.loose_ends.points.take().is_some() {
            return;
        }
        let n = self.highlight_loose_ends();
        self.model_status = Some(if n == 0 {
            ducad_i18n::t!("loose-ends-none")
        } else {
            ducad_i18n::t!("loose-ends-found", count = n)
        });
    }

    /// Lingkaran merah di setiap ujung terbuka.
    pub fn paint_loose_ends(&self, ui: &egui::Ui, rect: egui::Rect) {
        let Some(points) = &self.loose_ends.points else { return };
        let painter = ui.painter_at(rect);
        let red = egui::Color32::from_rgb(255, 69, 58);
        for p in points {
            let w = self.active_plane.to_world(*p, 0.0);
            if let Some(px) = world_to_screen_pos(&self.camera, rect, w) {
                painter.circle_stroke(px, 9.0, egui::Stroke::new(2.0, red));
                painter.circle_filled(px, 3.0, red);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::Entity;

    #[test]
    fn open_polyline_reports_two_loose_ends_and_toggles() {
        let mut app = DuCADApp::new_for_test();
        app.sketch_mut().entities.insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        app.sketch_mut().entities.insert(Entity::line(DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0)));
        app.toggle_loose_ends();
        let pts = app.loose_ends.points.clone().expect("disorot");
        assert_eq!(pts.len(), 2);
        assert!(pts.iter().any(|p| p.length() < 1e-9));
        app.toggle_loose_ends();
        assert!(app.loose_ends.points.is_none());
    }

    #[test]
    fn closed_triangle_has_no_loose_ends() {
        let mut app = DuCADApp::new_for_test();
        let p = [DVec2::ZERO, DVec2::new(10.0, 0.0), DVec2::new(0.0, 10.0)];
        for i in 0..3 {
            app.sketch_mut().entities.insert(Entity::line(p[i], p[(i + 1) % 3]));
        }
        assert_eq!(app.highlight_loose_ends(), 0);
        assert!(app.loose_ends.points.is_none());
    }
}
