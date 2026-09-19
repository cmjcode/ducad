//! Kartu error operasi GUI dengan fix terverifikasi (P9.3). Operasi GUI
//! memanggil `ducad_engine::compute` langsung (bukan `Session`), jadi
//! kandidat perbaikan diverifikasi di sini dengan compute yang sama pada
//! shape dan pilihan tepi/face yang sama, sebelum ditawarkan sebagai tombol.

use ducad_core::BodyId;
use ducad_engine::compute::{self, EdgePick, FacePick};
use ducad_engine::OpError;
use ducad_kernel::PickRay;
use ducad_ui::{ErrorCard, ErrorCardEvent};

use crate::app::DuCADApp;

/// Perbaikan yang bisa diterapkan tombol kartu error.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuiFix {
    FilletRadius(f64),
    ChamferDistance(f64),
    ShellThickness(f64),
}

/// Maksimum kandidat yang diverifikasi per error (sama dengan engine P9.2).
const MAX_VERIFY: usize = 3;

/// 0.9×, 0.5×, 0.25× nilai asal, dibulatkan ke bawah 0,1 mm.
fn candidates(value: f64) -> Vec<f64> {
    let mut out: Vec<f64> = [0.9, 0.5, 0.25]
        .iter()
        .map(|k| ((value * k) * 10.0).floor() / 10.0)
        .filter(|v| *v > 0.0)
        .collect();
    out.dedup();
    out.into_iter().take(MAX_VERIFY).collect()
}

impl DuCADApp {
    /// Tampilkan kartu error untuk `e`. Judul dari kunci i18n
    /// `error-<kode_snake_case>`; pesan `OpError.message` menjadi penyebab
    /// (dan fallback judul bila kunci tidak ada).
    pub fn show_op_error(&mut self, e: &OpError, fixes: Vec<(String, GuiFix)>) {
        let code = serde_json::to_value(e.code)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let key = format!("error-{code}");
        let title = ducad_i18n::translate(&key, None);
        let title = if title == key {
            ducad_i18n::t!("error-card-title-fallback")
        } else {
            title
        };
        self.error_card.open = true;
        self.error_card.title = title;
        self.error_card.cause = e.message.clone();
        self.error_card.fixes = fixes.iter().map(|(l, _)| l.clone()).collect();
        self.error_fixes = fixes.into_iter().map(|(_, f)| f).collect();
    }

    /// Kandidat radius/jarak yang lebih kecil untuk fillet/chamfer yang gagal,
    /// hanya yang benar-benar berhasil pada shape saat ini.
    pub fn round_fix_candidates(
        &self,
        id: BodyId,
        fillet: bool,
        value: f64,
        rays: &[PickRay],
    ) -> Vec<(String, GuiFix)> {
        let Some(geo) = self.model.geometry.get(id) else {
            return Vec::new();
        };
        let pick = if rays.is_empty() {
            EdgePick::All
        } else {
            EdgePick::Rays(rays, Self::EDGE_REAPPLY_TOLERANCE_MM)
        };
        candidates(value)
            .into_iter()
            .filter(|v| {
                if fillet {
                    compute::fillet(&geo.shape, &pick, *v).is_ok()
                } else {
                    compute::chamfer(&geo.shape, &pick, *v).is_ok()
                }
            })
            .map(|v| {
                if fillet {
                    (
                        ducad_i18n::t!("fix-use-radius", value = format!("{v}")),
                        GuiFix::FilletRadius(v),
                    )
                } else {
                    (
                        ducad_i18n::t!("fix-use-distance", value = format!("{v}")),
                        GuiFix::ChamferDistance(v),
                    )
                }
            })
            .collect()
    }

    /// Kandidat tebal shell yang lebih tipis dan terbukti berhasil.
    pub fn shell_fix_candidates(
        &self,
        id: BodyId,
        thickness: f64,
        faces: &[PickRay],
    ) -> Vec<(String, GuiFix)> {
        let Some(geo) = self.model.geometry.get(id) else {
            return Vec::new();
        };
        if faces.is_empty() {
            return Vec::new();
        }
        candidates(thickness)
            .into_iter()
            .filter(|v| compute::shell(&geo.shape, &FacePick::Rays(faces), *v).is_ok())
            .map(|v| {
                (
                    ducad_i18n::t!("fix-use-thickness", value = format!("{v}")),
                    GuiFix::ShellThickness(v),
                )
            })
            .collect()
    }

    /// Terapkan fix: isi field input terkait lalu jalankan ulang operasinya.
    pub fn apply_gui_fix(&mut self, i: usize) {
        let Some(fix) = self.error_fixes.get(i).copied() else {
            return;
        };
        self.error_card.open = false;
        match fix {
            GuiFix::FilletRadius(v) => {
                self.fillet_radius_input = format!("{v}");
                self.fillet_selected_body();
            }
            GuiFix::ChamferDistance(v) => {
                self.chamfer_distance_input = format!("{v}");
                self.chamfer_selected_body();
            }
            GuiFix::ShellThickness(v) => {
                self.shell_thickness_input = format!("{v}");
                self.shell_selected_body();
            }
        }
    }

    /// Render kartu error dan tangani tombolnya (dipanggil tiap frame).
    pub fn error_card_frame(&mut self, ctx: &egui::Context) {
        match ErrorCard::show(ctx, &self.error_card) {
            Some(ErrorCardEvent::ApplyFix(i)) => self.apply_gui_fix(i),
            Some(ErrorCardEvent::Close) => self.error_card.open = false,
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_fillet_offers_verified_fix_that_reapplies() {
        let mut app = DuCADApp::new_for_test();
        let geo = compute::primitive(
            &compute::PrimitiveShape::Box { size: [20.0, 20.0, 20.0], centered: false },
            [0.0; 3],
        )
        .unwrap();
        let id = app.model.doc.add_body("Box");
        app.model.geometry.insert(id, geo);
        app.selected_bodies = [id].into_iter().collect();
        app.fillet_radius_input = "15".into();
        app.fillet_selected_body();
        assert!(app.error_card.open, "fillet r=15 pada kubus 20 harus gagal");
        assert!(!app.error_card.fixes.is_empty(), "{:?}", app.error_card);
        assert!(!app.error_card.title.is_empty());
        let before = app.model.geometry[id].shape.volume();
        app.apply_gui_fix(0);
        assert!(!app.error_card.open);
        let id = *app.model.doc.bodies.keys().collect::<Vec<_>>().first().unwrap();
        assert!(app.model.geometry[id].shape.volume() < before, "fix diterapkan ulang");
    }

    #[test]
    fn candidates_are_rounded_down() {
        assert_eq!(candidates(3.0), vec![2.7, 1.5, 0.7]);
    }
}
