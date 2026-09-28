//! Tinta → entitas sketch CAD ("Jadikan Profil" / "Bentuk Pintar").
//!
//! Coretan tinta hanya jejak titik bertekanan; extrude butuh loop sketch
//! yang tertutup rapat. Modul ini merangkai pipeline yang sama dengan tool
//! Freehand — `recognize` → `infer_constraints` per coretan — lalu menutup
//! celah antar coretan dengan `gaps::plan_gap_closure`. Semua dikerjakan
//! pada SALINAN sketch; pemanggil GUI memasukkan hasilnya lewat command
//! agar bisa di-undo.

use std::collections::HashSet;

use ducad_sketch::constraint::types::Constraint;
use ducad_sketch::gaps::{apply_gap_fix, plan_gap_closure, GapOptions};
use ducad_sketch::infer::{infer_constraints, InferOptions};
use ducad_sketch::recognize::{recognize, to_entities, Stroke as RawStroke};
use ducad_sketch::{Entity, EntityId, Sketch};
use glam::DVec2;

use crate::stroke::Stroke;

/// Opsi konversi.
#[derive(Debug, Clone, Copy)]
pub struct VectorizeOptions {
    /// Jarak snap inferensi constraint (mm).
    pub snap_dist: f64,
    /// Celah terbesar antar ujung coretan yang ditutup (mm).
    pub close_gap: f64,
}

impl Default for VectorizeOptions {
    fn default() -> Self {
        Self {
            snap_dist: 2.0,
            close_gap: 3.0,
        }
    }
}

/// Hasil konversi, dirujuk dengan id sketch PERCOBAAN.
#[derive(Debug, Clone, Default)]
pub struct Vectorized {
    /// Entitas baru (termasuk garis jembatan celah), sejajar `trial_ids`.
    pub entities: Vec<Entity>,
    pub trial_ids: Vec<EntityId>,
    /// Constraint baru (antar entitas baru, atau ke entitas lama).
    pub constraints: Vec<Constraint>,
    /// Entitas LAMA yang ujungnya digeser penutup celah.
    pub updated_existing: Vec<(EntityId, Entity)>,
    /// Id coretan tinta yang berhasil dikenali.
    pub converted: Vec<u64>,
}

impl Vectorized {
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

/// Titik coretan sebagai koordinat bidang sketch f64 (mm).
pub fn stroke_points(stroke: &Stroke) -> Vec<DVec2> {
    stroke
        .points
        .iter()
        .map(|p| DVec2::new(p.x as f64, p.y as f64))
        .collect()
}

/// Ubah `strokes` menjadi entitas sketch di atas `base` (tidak diubah).
pub fn vectorize_strokes(base: &Sketch, strokes: &[&Stroke], opt: &VectorizeOptions) -> Vectorized {
    let mut trial = base.clone();
    let base_constraints = trial.constraints.len();
    let mut new_ids: Vec<EntityId> = Vec::new();
    let mut converted = Vec::new();
    let infer_opt = InferOptions {
        snap_dist: opt.snap_dist,
        ..InferOptions::default()
    };

    for stroke in strokes {
        let raw = RawStroke {
            points: stroke_points(stroke),
            pressure: stroke.points.iter().map(|p| p.pressure).collect(),
        };
        let Some(shape) = recognize(&raw) else {
            continue;
        };
        let entities = to_entities(&shape);
        if entities.is_empty() {
            continue;
        }
        let ids: Vec<EntityId> = entities
            .into_iter()
            .map(|e| trial.entities.insert(e))
            .collect();
        trial = infer_constraints(&trial, &ids, &infer_opt).sketch;
        new_ids.extend(ids);
        converted.push(stroke.id);
    }
    if new_ids.is_empty() {
        return Vectorized::default();
    }

    let scope: HashSet<EntityId> = new_ids.iter().copied().collect();
    let fix = plan_gap_closure(
        &trial,
        Some(&scope),
        &GapOptions {
            max_gap: opt.close_gap,
            move_max: opt.close_gap,
        },
    );
    new_ids.extend(apply_gap_fix(&mut trial, &fix));

    let updated_existing = base
        .entities
        .iter()
        .filter_map(|(id, old)| {
            let now = trial.entities.get(id)?;
            (now != old).then(|| (id, now.clone()))
        })
        .collect();
    Vectorized {
        entities: new_ids
            .iter()
            .filter_map(|id| trial.entities.get(*id).cloned())
            .collect(),
        trial_ids: new_ids,
        constraints: trial.constraints.split_off(base_constraints),
        updated_existing,
        converted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::InkPoint;
    use ducad_sketch::find_closed_regions;
    use ducad_sketch::layer::LayerId;
    use ducad_sketch::style::Rgba;

    fn ink(id: u64, pts: &[DVec2]) -> Stroke {
        Stroke::new(
            id,
            pts.iter()
                .map(|p| InkPoint::new(p.x as f32, p.y as f32, 0.5, 0.0, 0))
                .collect(),
            Default::default(),
            Rgba::BLACK,
            LayerId::default(),
        )
    }

    fn segment(a: DVec2, b: DVec2) -> Vec<DVec2> {
        (0..=30).map(|i| a + (b - a) * (i as f64 / 30.0)).collect()
    }

    fn arc(center: DVec2, r: f64, from: f64, to: f64) -> Vec<DVec2> {
        (0..=60)
            .map(|i| center + DVec2::from_angle(from + (to - from) * i as f64 / 60.0) * r)
            .collect()
    }

    fn build(base: &Sketch, v: &Vectorized) -> Sketch {
        let mut s = base.clone();
        for (id, e) in &v.updated_existing {
            s.entities[*id] = e.clone();
        }
        for e in &v.entities {
            s.entities.insert(e.clone());
        }
        s
    }

    #[test]
    fn four_sloppy_strokes_make_one_region() {
        // Empat coretan garis yang ujungnya tidak bertemu (celah ±1 mm).
        let strokes = [
            ink(1, &segment(DVec2::new(0.0, 0.0), DVec2::new(40.0, 0.4))),
            ink(2, &segment(DVec2::new(40.8, 1.0), DVec2::new(40.3, 25.0))),
            ink(3, &segment(DVec2::new(39.2, 25.6), DVec2::new(0.5, 25.2))),
            ink(4, &segment(DVec2::new(-0.6, 24.3), DVec2::new(-0.4, 1.1))),
        ];
        let refs: Vec<&Stroke> = strokes.iter().collect();
        let base = Sketch::default();
        let v = vectorize_strokes(&base, &refs, &VectorizeOptions::default());
        assert_eq!(v.converted, vec![1, 2, 3, 4]);
        assert!(!v.constraints.is_empty());
        let s = build(&base, &v);
        let regions = find_closed_regions(&s);
        assert_eq!(regions.len(), 1, "{:?}", v.entities);
        assert!(
            (regions[0].area - 1000.0).abs() < 80.0,
            "luas {}",
            regions[0].area
        );
    }

    #[test]
    fn two_arc_strokes_close_into_region() {
        use std::f64::consts::PI;
        let strokes = [
            ink(7, &arc(DVec2::ZERO, 15.0, 0.08, PI - 0.08)),
            ink(8, &arc(DVec2::ZERO, 15.0, PI + 0.08, 2.0 * PI - 0.08)),
        ];
        let refs: Vec<&Stroke> = strokes.iter().collect();
        let base = Sketch::default();
        let v = vectorize_strokes(&base, &refs, &VectorizeOptions::default());
        assert_eq!(v.converted.len(), 2);
        assert_eq!(find_closed_regions(&build(&base, &v)).len(), 1);
    }

    #[test]
    fn tap_stroke_is_skipped() {
        let s = ink(3, &[DVec2::ZERO]);
        let v = vectorize_strokes(&Sketch::default(), &[&s], &VectorizeOptions::default());
        assert!(v.is_empty());
        assert!(v.converted.is_empty());
    }
}
