//! Align & Distribute untuk entitas vektor dalam sketch (M2.5).
//!
//! Menghitung translasi per entitas dan menghasilkan command `TranslateEntities`.

use ducad_sketch::commands::TranslateEntities;
use ducad_sketch::entity::EntityId;
use ducad_sketch::Sketch;
use glam::DVec2;

/// Arah perataan posisi objek.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    Left,
    CenterHorizontal,
    Right,
    Top,
    CenterVertical,
    Bottom,
}

/// Referensi acuan perataan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignRelative {
    #[default]
    Selection,
    Page,
    KeyObject(EntityId),
}

/// Mode distribusi jarak antar objek.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributeMode {
    HorizontalCenters,
    VerticalCenters,
    HorizontalGaps,
    VerticalGaps,
}

pub struct AlignTool;

impl AlignTool {
    /// Menghitung batas target (target bbox) berdasarkan acuan relatif yang dipilih.
    pub fn compute_target_bounds(
        sketch: &Sketch,
        selected_ids: &[EntityId],
        relative: AlignRelative,
    ) -> Option<(DVec2, DVec2)> {
        match relative {
            AlignRelative::Selection => {
                let mut min_bound = DVec2::splat(f64::INFINITY);
                let mut max_bound = DVec2::splat(f64::NEG_INFINITY);
                let mut found = false;

                for &id in selected_ids {
                    if let Some(e) = sketch.entities.get(id) {
                        if let Some((bmin, bmax)) = e.bounding_box() {
                            min_bound = min_bound.min(bmin);
                            max_bound = max_bound.max(bmax);
                            found = true;
                        }
                    }
                }
                if found {
                    Some((min_bound, max_bound))
                } else {
                    None
                }
            }
            AlignRelative::Page => sketch.bounding_box(),
            AlignRelative::KeyObject(key_id) => {
                sketch.entities.get(key_id).and_then(|e| e.bounding_box())
            }
        }
    }

    /// Menghasilkan command `TranslateEntities` untuk meratakan (align) entitas terpilih.
    pub fn align(
        sketch: &Sketch,
        selected_ids: &[EntityId],
        mode: AlignMode,
        relative: AlignRelative,
    ) -> Option<TranslateEntities> {
        if selected_ids.is_empty() {
            return None;
        }

        let (t_min, t_max) = Self::compute_target_bounds(sketch, selected_ids, relative)?;
        let t_mid = (t_min + t_max) * 0.5;

        let mut deltas = Vec::new();

        for &id in selected_ids {
            let entity = sketch.entities.get(id)?;
            let (emin, emax) = entity.bounding_box()?;
            let emid = (emin + emax) * 0.5;

            let delta = match mode {
                AlignMode::Left => DVec2::new(t_min.x - emin.x, 0.0),
                AlignMode::CenterHorizontal => DVec2::new(t_mid.x - emid.x, 0.0),
                AlignMode::Right => DVec2::new(t_max.x - emax.x, 0.0),
                AlignMode::Top => DVec2::new(0.0, t_max.y - emax.y),
                AlignMode::CenterVertical => DVec2::new(0.0, t_mid.y - emid.y),
                AlignMode::Bottom => DVec2::new(0.0, t_min.y - emin.y),
            };

            deltas.push((id, delta));
        }

        let label = match mode {
            AlignMode::Left => "Rata Kiri",
            AlignMode::CenterHorizontal => "Rata Tengah Horizontal",
            AlignMode::Right => "Rata Kanan",
            AlignMode::Top => "Rata Atas",
            AlignMode::CenterVertical => "Rata Tengah Vertikal",
            AlignMode::Bottom => "Rata Bawah",
        };

        Some(TranslateEntities::with_deltas(label, deltas))
    }

    /// Menghasilkan command `TranslateEntities` untuk mendistribusikan jarak antar entitas terpilih secara merata.
    pub fn distribute(
        sketch: &Sketch,
        selected_ids: &[EntityId],
        mode: DistributeMode,
    ) -> Option<TranslateEntities> {
        if selected_ids.len() < 3 {
            return None;
        }

        let mut items = Vec::new();
        for &id in selected_ids {
            let entity = sketch.entities.get(id)?;
            let (emin, emax) = entity.bounding_box()?;
            let emid = (emin + emax) * 0.5;
            items.push((id, emin, emax, emid));
        }

        let mut deltas = Vec::new();

        match mode {
            DistributeMode::HorizontalCenters => {
                items.sort_by(|a, b| a.3.x.partial_cmp(&b.3.x).unwrap_or(std::cmp::Ordering::Equal));
                let first_cx = items.first().unwrap().3.x;
                let last_cx = items.last().unwrap().3.x;
                let step = (last_cx - first_cx) / (items.len() - 1) as f64;

                for (i, &(id, _, _, cur_mid)) in items.iter().enumerate() {
                    let target_cx = first_cx + (i as f64) * step;
                    deltas.push((id, DVec2::new(target_cx - cur_mid.x, 0.0)));
                }
            }
            DistributeMode::VerticalCenters => {
                items.sort_by(|a, b| a.3.y.partial_cmp(&b.3.y).unwrap_or(std::cmp::Ordering::Equal));
                let first_cy = items.first().unwrap().3.y;
                let last_cy = items.last().unwrap().3.y;
                let step = (last_cy - first_cy) / (items.len() - 1) as f64;

                for (i, &(id, _, _, cur_mid)) in items.iter().enumerate() {
                    let target_cy = first_cy + (i as f64) * step;
                    deltas.push((id, DVec2::new(0.0, target_cy - cur_mid.y)));
                }
            }
            DistributeMode::HorizontalGaps => {
                items.sort_by(|a, b| a.1.x.partial_cmp(&b.1.x).unwrap_or(std::cmp::Ordering::Equal));
                let total_item_width: f64 = items.iter().map(|(_, emin, emax, _)| emax.x - emin.x).sum();
                let first_min_x = items.first().unwrap().1.x;
                let last_max_x = items.last().unwrap().2.x;
                let total_span = last_max_x - first_min_x;
                let total_gap = total_span - total_item_width;
                let gap = (total_gap / (items.len() - 1) as f64).max(0.0);

                let mut current_left = first_min_x;
                for &(id, emin, emax, _) in &items {
                    let w = emax.x - emin.x;
                    deltas.push((id, DVec2::new(current_left - emin.x, 0.0)));
                    current_left += w + gap;
                }
            }
            DistributeMode::VerticalGaps => {
                items.sort_by(|a, b| a.1.y.partial_cmp(&b.1.y).unwrap_or(std::cmp::Ordering::Equal));
                let total_item_height: f64 = items.iter().map(|(_, emin, emax, _)| emax.y - emin.y).sum();
                let first_min_y = items.first().unwrap().1.y;
                let last_max_y = items.last().unwrap().2.y;
                let total_span = last_max_y - first_min_y;
                let total_gap = total_span - total_item_height;
                let gap = (total_gap / (items.len() - 1) as f64).max(0.0);

                let mut current_bottom = first_min_y;
                for &(id, emin, emax, _) in &items {
                    let h = emax.y - emin.y;
                    deltas.push((id, DVec2::new(0.0, current_bottom - emin.y)));
                    current_bottom += h + gap;
                }
            }
        }

        let label = match mode {
            DistributeMode::HorizontalCenters => "Distribusi Pusat Horizontal",
            DistributeMode::VerticalCenters => "Distribusi Pusat Vertikal",
            DistributeMode::HorizontalGaps => "Distribusi Jarak Horizontal",
            DistributeMode::VerticalGaps => "Distribusi Jarak Vertikal",
        };

        Some(TranslateEntities::with_deltas(label, deltas))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::Command;
    use ducad_sketch::entity::Entity;
    use ducad_sketch::path_edit::shape_to_path_rect;

    #[test]
    fn align_left_moves_all_to_min_x() {
        let mut sketch = Sketch::default();
        let r1 = shape_to_path_rect(DVec2::new(10.0, 0.0), DVec2::new(20.0, 10.0));
        let id1 = sketch.entities.insert(Entity::Path {
            subpaths: vec![r1],
            is_construction: false,
        });

        let r2 = shape_to_path_rect(DVec2::new(30.0, 0.0), DVec2::new(40.0, 10.0));
        let id2 = sketch.entities.insert(Entity::Path {
            subpaths: vec![r2],
            is_construction: false,
        });

        let r3 = shape_to_path_rect(DVec2::new(50.0, 0.0), DVec2::new(60.0, 10.0));
        let id3 = sketch.entities.insert(Entity::Path {
            subpaths: vec![r3],
            is_construction: false,
        });

        let mut cmd = AlignTool::align(&sketch, &[id1, id2, id3], AlignMode::Left, AlignRelative::Selection)
            .expect("align left should succeed");
        cmd.apply(&mut sketch);

        for &id in &[id1, id2, id3] {
            let (bmin, _) = sketch.entities.get(id).unwrap().bounding_box().unwrap();
            assert!((bmin.x - 10.0).abs() < 1e-9, "Expected min.x 10.0, got {}", bmin.x);
        }
    }

    #[test]
    fn distribute_centers_equal_spacing() {
        let mut sketch = Sketch::default();
        let r1 = shape_to_path_rect(DVec2::new(-5.0, -5.0), DVec2::new(5.0, 5.0)); // center x = 0
        let id1 = sketch.entities.insert(Entity::Path {
            subpaths: vec![r1],
            is_construction: false,
        });

        let r2 = shape_to_path_rect(DVec2::new(5.0, -5.0), DVec2::new(15.0, 5.0)); // center x = 10
        let id2 = sketch.entities.insert(Entity::Path {
            subpaths: vec![r2],
            is_construction: false,
        });

        let r3 = shape_to_path_rect(DVec2::new(95.0, -5.0), DVec2::new(105.0, 5.0)); // center x = 100
        let id3 = sketch.entities.insert(Entity::Path {
            subpaths: vec![r3],
            is_construction: false,
        });

        let mut cmd = AlignTool::distribute(&sketch, &[id1, id2, id3], DistributeMode::HorizontalCenters)
            .expect("distribute centers should succeed");
        cmd.apply(&mut sketch);

        let c1 = sketch.entities.get(id1).unwrap().bounding_box().map(|(min, max)| (min.x + max.x) * 0.5).unwrap();
        let c2 = sketch.entities.get(id2).unwrap().bounding_box().map(|(min, max)| (min.x + max.x) * 0.5).unwrap();
        let c3 = sketch.entities.get(id3).unwrap().bounding_box().map(|(min, max)| (min.x + max.x) * 0.5).unwrap();

        assert!((c1 - 0.0).abs() < 1e-9, "Expected center 0.0, got {c1}");
        assert!((c2 - 50.0).abs() < 1e-9, "Expected center 50.0, got {c2}");
        assert!((c3 - 100.0).abs() < 1e-9, "Expected center 100.0, got {c3}");
    }
}
