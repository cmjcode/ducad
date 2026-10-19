//! Mode gambar tambahan (Paritas 0.6): Busur dari titik pusat dan Persegi
//! dari titik pusat. Dipilih lewat HUD atas saat tool Arc/Rectangle aktif.

use ducad_sketch::{arc_from_center, arc_from_three_points, rect_opposite_corner_from_center, Entity};
use ducad_ui::CanvasHud;
use glam::DVec2;

use crate::app::DuCADApp;
use crate::types::ToolKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArcDrawMode {
    /// Awal, akhir, titik pada busur.
    #[default]
    ThreePoint,
    /// Pusat, awal (radius), akhir (sudut).
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RectDrawMode {
    /// Dua sudut berseberangan.
    #[default]
    Corner,
    /// Pusat lalu satu sudut.
    Center,
}

#[derive(Default)]
pub struct SketchModes {
    pub arc: ArcDrawMode,
    pub rect: RectDrawMode,
    /// Jarak kedua chamfer 2D; `None` = sama dengan jarak pertama (Paritas 0.7).
    pub chamfer_d2: Option<f64>,
}

/// Input dinamis chamfer: `"5"` (simetris) atau `"5,3"` / `"5x3"` / `"5;3"`
/// (dua jarak). `None` bila tidak valid.
pub fn parse_chamfer_input(text: &str) -> Option<(f64, Option<f64>)> {
    let parts: Vec<&str> = text.split([',', ';', 'x', 'X']).map(str::trim).collect();
    let num = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0);
    match parts.as_slice() {
        [a] => Some((num(a)?, None)),
        [a, b] => Some((num(a)?, Some(num(b)?))),
        _ => None,
    }
}

impl DuCADApp {
    /// Jarak kedua chamfer 2D yang berlaku.
    pub fn chamfer_second_dist(&self) -> f64 {
        self.sketch_modes.chamfer_d2.unwrap_or(self.chamfer_2d_dist)
    }

    /// Sudut pertama persegi untuk min/max: `first` apa adanya di mode
    /// Corner, atau cerminan `second` terhadap pusat di mode Center.
    pub fn rect_first_corner(&self, first: DVec2, second: DVec2) -> DVec2 {
        match self.sketch_modes.rect {
            RectDrawMode::Corner => first,
            RectDrawMode::Center => rect_opposite_corner_from_center(first, second),
        }
    }

    /// Busur dari tiga titik klik sesuai mode aktif.
    pub fn arc_from_clicks(&self, p0: DVec2, p1: DVec2, p2: DVec2) -> Option<Entity> {
        match self.sketch_modes.arc {
            ArcDrawMode::ThreePoint => arc_from_three_points(p0, p1, p2),
            ArcDrawMode::Center => arc_from_center(p0, p1, p2),
        }
    }

    /// HUD pilihan mode untuk tool Arc/Rectangle. `true` bila HUD tampil.
    pub fn render_draw_mode_hud(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> bool {
        let (title, options): (String, [(String, bool); 2]) = match self.tool {
            ToolKind::Arc => (
                ducad_i18n::t!("hud-arc-mode"),
                [
                    (ducad_i18n::t!("hud-arc-three-point"), self.sketch_modes.arc == ArcDrawMode::ThreePoint),
                    (ducad_i18n::t!("hud-arc-center"), self.sketch_modes.arc == ArcDrawMode::Center),
                ],
            ),
            ToolKind::Rectangle => (
                ducad_i18n::t!("hud-rect-mode"),
                [
                    (ducad_i18n::t!("hud-rect-corner"), self.sketch_modes.rect == RectDrawMode::Corner),
                    (ducad_i18n::t!("hud-rect-center"), self.sketch_modes.rect == RectDrawMode::Center),
                ],
            ),
            ToolKind::Chamfer2D => {
                self.render_chamfer_hud(ui, rect);
                return true;
            }
            _ => return false,
        };
        let picked = CanvasHud::render_header_hud_container(ui, rect, 420.0, true, "ducad-hud-draw-mode", |ui| {
            CanvasHud::hud_title(ui, &title, true);
            ui.separator();
            let mut picked = None;
            for (i, (label, active)) in options.iter().enumerate() {
                if CanvasHud::hud_toggle_btn(ui, label, *active).clicked() {
                    picked = Some(i);
                }
            }
            picked
        });
        if let Some(i) = picked {
            // Ganti mode membuang titik tertunda supaya klik lama tidak ditafsir ulang.
            self.pending_points.clear();
            match self.tool {
                ToolKind::Arc => {
                    self.sketch_modes.arc = if i == 0 { ArcDrawMode::ThreePoint } else { ArcDrawMode::Center }
                }
                _ => self.sketch_modes.rect = if i == 0 { RectDrawMode::Corner } else { RectDrawMode::Center },
            }
        }
        true
    }
}

impl DuCADApp {
    fn render_chamfer_hud(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let mut d1 = self.chamfer_2d_dist;
        let mut equal = self.sketch_modes.chamfer_d2.is_none();
        let mut d2 = self.chamfer_second_dist();
        CanvasHud::render_header_hud_container(ui, rect, 460.0, true, "ducad-hud-chamfer2d", |ui| {
            CanvasHud::hud_title(ui, ducad_i18n::t!("hud-chamfer-2d"), true);
            ui.separator();
            ui.label(ducad_i18n::t!("hud-chamfer-d1"));
            ui.add(egui::DragValue::new(&mut d1).speed(0.1).range(0.01..=10_000.0));
            ui.checkbox(&mut equal, ducad_i18n::t!("hud-chamfer-equal"));
            if !equal {
                ui.label(ducad_i18n::t!("hud-chamfer-d2"));
                ui.add(egui::DragValue::new(&mut d2).speed(0.1).range(0.01..=10_000.0));
            }
        });
        self.chamfer_2d_dist = d1;
        self.sketch_modes.chamfer_d2 = (!equal).then_some(d2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chamfer_input_parses_one_or_two_distances() {
        assert_eq!(parse_chamfer_input("5"), Some((5.0, None)));
        assert_eq!(parse_chamfer_input(" 5 , 3 "), Some((5.0, Some(3.0))));
        assert_eq!(parse_chamfer_input("4x2"), Some((4.0, Some(2.0))));
        assert_eq!(parse_chamfer_input(""), None);
        assert_eq!(parse_chamfer_input("-1"), None);
        assert_eq!(parse_chamfer_input("NaN"), None);
        assert_eq!(parse_chamfer_input("1,2,3"), None);
    }

    #[test]
    fn two_distance_chamfer_cuts_unequal_legs() {
        let mut app = DuCADApp::new_for_test();
        app.chamfer_2d_dist = 4.0;
        app.sketch_modes.chamfer_d2 = Some(2.0);
        let res = ducad_sketch::compute_chamfer_2d(
            (DVec2::new(10.0, 0.0), DVec2::ZERO),
            (DVec2::ZERO, DVec2::new(0.0, 10.0)),
            app.chamfer_2d_dist,
            app.chamfer_second_dist(),
        )
        .expect("chamfer");
        let Entity::Line { start, end, .. } = res.bevel_line else { panic!("bukan garis") };
        let legs = [start.length(), end.length()];
        assert!(legs.iter().any(|l| (l - 4.0).abs() < 1e-9) && legs.iter().any(|l| (l - 2.0).abs() < 1e-9), "{legs:?}");
    }

    #[test]
    fn center_rect_mode_mirrors_first_corner() {
        let mut app = DuCADApp::new_for_test();
        let (c, k) = (DVec2::new(10.0, 10.0), DVec2::new(14.0, 12.0));
        assert_eq!(app.rect_first_corner(c, k), c);
        app.sketch_modes.rect = RectDrawMode::Center;
        assert_eq!(app.rect_first_corner(c, k), DVec2::new(6.0, 8.0));
    }

    #[test]
    fn center_arc_mode_uses_first_click_as_center() {
        let mut app = DuCADApp::new_for_test();
        app.sketch_modes.arc = ArcDrawMode::Center;
        let arc = app.arc_from_clicks(DVec2::ZERO, DVec2::new(3.0, 0.0), DVec2::new(-1.0, 0.0)).unwrap();
        let Entity::Arc { center, radius, .. } = arc else { panic!("bukan busur") };
        assert_eq!(center, DVec2::ZERO);
        assert!((radius - 3.0).abs() < 1e-12);
    }
}
