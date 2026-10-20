//! Mode gambar tambahan (Paritas 0.6 + Fase 1B 1.7–1.12): Busur dari titik
//! pusat/tangen, Persegi dari pusat/3 titik/sudut bulat, Lingkaran 2 titik/3
//! titik/2-tangen/3-tangen, Garis titik-tengah/tak hingga. Dipilih lewat HUD
//! atas saat tool terkait aktif; logika klik/preview ada di
//! `sketch_modes_fase1.rs`.

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
    /// Awal menempel ujung Line/Arc, busur menyinggungnya (Paritas 1.8).
    Tangent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RectDrawMode {
    /// Dua sudut berseberangan.
    #[default]
    Corner,
    /// Pusat lalu satu sudut.
    Center,
    /// Sisi p0→p1 lalu lebar (boleh miring) (Paritas 1.9).
    ThreePoint,
    /// Dua sudut + radius sudut dari HUD; 4 garis + 4 busur (Paritas 1.9).
    Rounded,
}

/// Mode tool Lingkaran (Paritas 1.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircleDrawMode {
    /// Pusat lalu radius.
    #[default]
    CenterRadius,
    /// Dua ujung diameter.
    TwoPoint,
    /// Tiga titik pada keliling.
    ThreePoint,
    /// Klik dua kurva (garis/lingkaran/busur), lalu kursor menentukan radius.
    TwoTangent,
    /// Klik tiga kurva; lingkaran menyinggung ketiganya (constraint Tangent ×3).
    ThreeTangent,
}

/// Mode tool Garis (Paritas 1.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineDrawMode {
    /// Garis berantai biasa.
    #[default]
    Chain,
    /// Klik pertama = titik tengah, klik kedua = salah satu ujung.
    Midpoint,
    /// Garis konstruksi tak hingga lewat dua titik.
    Infinite,
}

/// Salinan mode aktif agar HUD bisa memanggil `&mut self` tanpa konflik pinjam.
struct DrawModeSnapshot {
    arc: ArcDrawMode,
    rect: RectDrawMode,
    circle: CircleDrawMode,
    line: LineDrawMode,
}

impl DrawModeSnapshot {
    fn of(m: &SketchModes) -> Self {
        Self { arc: m.arc, rect: m.rect, circle: m.circle, line: m.line }
    }
}

pub struct SketchModes {
    pub arc: ArcDrawMode,
    pub rect: RectDrawMode,
    pub circle: CircleDrawMode,
    pub line: LineDrawMode,
    /// Radius sudut persegi bulat (mm).
    pub corner_radius: f64,
    /// Toleransi pilih (satuan dunia) dari klik terakhir — dipakai mode
    /// tangen untuk mencari kurva/ujung yang diklik.
    pub pick_tol: f64,
    /// Jarak kedua chamfer 2D; `None` = sama dengan jarak pertama (Paritas 0.7).
    pub chamfer_d2: Option<f64>,
}

impl Default for SketchModes {
    fn default() -> Self {
        Self {
            arc: ArcDrawMode::default(),
            rect: RectDrawMode::default(),
            circle: CircleDrawMode::default(),
            line: LineDrawMode::default(),
            corner_radius: 2.0,
            pick_tol: 1.0,
            chamfer_d2: None,
        }
    }
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
            RectDrawMode::Corner | RectDrawMode::ThreePoint | RectDrawMode::Rounded => first,
            RectDrawMode::Center => rect_opposite_corner_from_center(first, second),
        }
    }

    /// Busur dari tiga titik klik sesuai mode aktif.
    pub fn arc_from_clicks(&self, p0: DVec2, p1: DVec2, p2: DVec2) -> Option<Entity> {
        match self.sketch_modes.arc {
            ArcDrawMode::ThreePoint => arc_from_three_points(p0, p1, p2),
            ArcDrawMode::Center => arc_from_center(p0, p1, p2),
            // Busur tangen hanya 2 klik — lihat `finish_mode_multipoint`.
            ArcDrawMode::Tangent => None,
        }
    }

    /// HUD pilihan mode untuk tool Arc/Rectangle/Circle/Line. `true` bila HUD tampil.
    pub fn render_draw_mode_hud(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> bool {
        let m = DrawModeSnapshot::of(&self.sketch_modes);
        let (title, options): (String, Vec<(String, bool)>) = match self.tool {
            ToolKind::Arc => (
                ducad_i18n::t!("hud-arc-mode"),
                vec![
                    (ducad_i18n::t!("hud-arc-three-point"), m.arc == ArcDrawMode::ThreePoint),
                    (ducad_i18n::t!("hud-arc-center"), m.arc == ArcDrawMode::Center),
                    (ducad_i18n::t!("hud-arc-tangent"), m.arc == ArcDrawMode::Tangent),
                ],
            ),
            ToolKind::Rectangle => (
                ducad_i18n::t!("hud-rect-mode"),
                vec![
                    (ducad_i18n::t!("hud-rect-corner"), m.rect == RectDrawMode::Corner),
                    (ducad_i18n::t!("hud-rect-center"), m.rect == RectDrawMode::Center),
                    (ducad_i18n::t!("hud-rect-three-point"), m.rect == RectDrawMode::ThreePoint),
                    (ducad_i18n::t!("hud-rect-rounded"), m.rect == RectDrawMode::Rounded),
                ],
            ),
            ToolKind::Circle => (
                ducad_i18n::t!("hud-circle-mode"),
                vec![
                    (ducad_i18n::t!("hud-circle-center"), m.circle == CircleDrawMode::CenterRadius),
                    (ducad_i18n::t!("hud-circle-two-point"), m.circle == CircleDrawMode::TwoPoint),
                    (ducad_i18n::t!("hud-circle-three-point"), m.circle == CircleDrawMode::ThreePoint),
                    (ducad_i18n::t!("hud-circle-two-tangent"), m.circle == CircleDrawMode::TwoTangent),
                    (ducad_i18n::t!("hud-circle-three-tangent"), m.circle == CircleDrawMode::ThreeTangent),
                ],
            ),
            ToolKind::Line => (
                ducad_i18n::t!("hud-line-mode"),
                vec![
                    (ducad_i18n::t!("hud-line-chain"), m.line == LineDrawMode::Chain),
                    (ducad_i18n::t!("hud-line-midpoint"), m.line == LineDrawMode::Midpoint),
                    (ducad_i18n::t!("hud-line-infinite"), m.line == LineDrawMode::Infinite),
                ],
            ),
            ToolKind::Chamfer2D => {
                self.render_chamfer_hud(ui, rect);
                return true;
            }
            _ => return false,
        };
        let show_radius = self.tool == ToolKind::Rectangle && m.rect == RectDrawMode::Rounded;
        let mut radius = self.sketch_modes.corner_radius;
        let width = 160.0 + 110.0 * options.len() as f32 + if show_radius { 150.0 } else { 0.0 };
        let picked = CanvasHud::render_header_hud_container(ui, rect, width, true, "ducad-hud-draw-mode", |ui| {
            CanvasHud::hud_title(ui, &title, true);
            ui.separator();
            let mut picked = None;
            for (i, (label, active)) in options.iter().enumerate() {
                if CanvasHud::hud_toggle_btn(ui, label, *active).clicked() {
                    picked = Some(i);
                }
            }
            if show_radius {
                ui.separator();
                ui.label(ducad_i18n::t!("hud-rect-corner-radius"));
                ui.add(egui::DragValue::new(&mut radius).speed(0.1).range(0.01..=10_000.0));
            }
            picked
        });
        if radius.is_finite() && radius > 0.0 {
            self.sketch_modes.corner_radius = radius;
        }
        if let Some(i) = picked {
            self.set_draw_mode_index(i);
        }
        true
    }

    /// Pilih mode ke-`i` dari HUD tool aktif. Ganti mode membuang titik
    /// tertunda supaya klik lama tidak ditafsir ulang.
    pub fn set_draw_mode_index(&mut self, i: usize) {
        self.cancel_pending_points();
        let m = &mut self.sketch_modes;
        match self.tool {
            ToolKind::Arc => {
                m.arc = [ArcDrawMode::ThreePoint, ArcDrawMode::Center, ArcDrawMode::Tangent]
                    .get(i)
                    .copied()
                    .unwrap_or_default()
            }
            ToolKind::Rectangle => {
                m.rect = [RectDrawMode::Corner, RectDrawMode::Center, RectDrawMode::ThreePoint, RectDrawMode::Rounded]
                    .get(i)
                    .copied()
                    .unwrap_or_default()
            }
            ToolKind::Circle => {
                m.circle = [
                    CircleDrawMode::CenterRadius,
                    CircleDrawMode::TwoPoint,
                    CircleDrawMode::ThreePoint,
                    CircleDrawMode::TwoTangent,
                    CircleDrawMode::ThreeTangent,
                ]
                .get(i)
                .copied()
                .unwrap_or_default()
            }
            ToolKind::Line => {
                m.line = [LineDrawMode::Chain, LineDrawMode::Midpoint, LineDrawMode::Infinite]
                    .get(i)
                    .copied()
                    .unwrap_or_default()
            }
            _ => {}
        }
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
