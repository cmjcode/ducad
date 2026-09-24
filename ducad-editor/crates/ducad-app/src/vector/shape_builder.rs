//! Shape Builder tool dan operasi boolean interaktif (M2.4).
//!
//! Menggabungkan, memotong, mengiris, dan mengecualikan path vektor pada sketch.
//! Objek baru mewarisi style, layer, dan nama dari objek paling bawah (target object),
//! dan dieksekusi dalam satu transaksi undo `ReplaceEntities`.

use ducad_sketch::commands::ReplaceEntities;
use ducad_sketch::entity::{Entity, EntityId, PathSeg, Subpath};
use ducad_sketch::path_edit::{shape_to_path_circle, shape_to_path_ellipse};
use ducad_sketch::path_ops::{boolean, BoolOp, PathOpError};
use ducad_sketch::style::FillRule;
use ducad_sketch::Sketch;

pub struct ShapeBuilder;

impl ShapeBuilder {
    /// Mengonversi sembarang entitas sketch menjadi sekumpulan subpath.
    pub fn entity_to_subpaths(entity: &Entity) -> Vec<Subpath> {
        match entity {
            Entity::Path { subpaths, .. } => subpaths.clone(),
            Entity::Circle { center, radius, .. } => {
                vec![shape_to_path_circle(*center, *radius)]
            }
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
                ..
            } => {
                vec![shape_to_path_ellipse(*center, *radius_x, *radius_y)]
            }
            Entity::Line { start, end, .. } => {
                vec![Subpath {
                    start: *start,
                    segs: vec![PathSeg::Line { end: *end }],
                    closed: false,
                }]
            }
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let arc = ducad_sketch::kurbo::Arc {
                    center: ducad_sketch::kurbo::Point::new(center.x, center.y),
                    radii: ducad_sketch::kurbo::Vec2::new(*radius, *radius),
                    start_angle: *start_angle,
                    sweep_angle: end_angle - start_angle,
                    x_rotation: 0.0,
                };
                let bez = ducad_sketch::kurbo::Shape::to_path(&arc, 0.01);
                Subpath::from_kurbo(&bez)
            }
            Entity::Spline { points, exact, .. } => {
                if let Some(exact_segs) = exact {
                    if let Some(&start) = points.first() {
                        vec![Subpath {
                            start,
                            segs: exact_segs.clone(),
                            closed: false,
                        }]
                    } else {
                        Vec::new()
                    }
                } else if points.len() >= 2 {
                    let mut segs = Vec::with_capacity(points.len() - 1);
                    for &p in &points[1..] {
                        segs.push(PathSeg::Line { end: p });
                    }
                    vec![Subpath {
                        start: points[0],
                        segs,
                        closed: false,
                    }]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Membangun command `ReplaceEntities` dari operasi boolean antar entitas yang dipilih.
    ///
    /// Urutan objek ditentukan berdasarkan `sketch.draw_order()` (bawah ke atas):
    /// - Objek paling bawah adalah target object yang style, layer, dan namanya diwarisi.
    /// - Objek atas memotong (Difference) atau digabung (Union/Intersection/Xor) dengan objek bawah.
    /// - Seluruh entitas sumber dihapus dan digantikan oleh satu `Entity::Path` baru.
    pub fn build_boolean_command(
        sketch: &Sketch,
        selected_ids: &[EntityId],
        op: BoolOp,
        tol: f64,
    ) -> Result<ReplaceEntities, PathOpError> {
        if selected_ids.len() < 2 {
            return Err(PathOpError::Empty);
        }

        // Urutkan ID sesuai urutan gambar sketch (bawah ke atas)
        let draw_order = sketch.draw_order();
        let mut ordered_ids: Vec<EntityId> = selected_ids.to_vec();
        ordered_ids.sort_by_key(|id| {
            draw_order.iter().position(|x| x == id).unwrap_or(usize::MAX)
        });

        let target_id = ordered_ids[0];
        let target_entity = sketch.entities.get(target_id).ok_or(PathOpError::Empty)?;
        let target_style = sketch.styles.get(target_id).cloned();
        let target_layer = sketch.entity_layer.get(target_id).copied();

        let fill_rule = target_style
            .as_ref()
            .map(|s| s.fill_rule)
            .unwrap_or(FillRule::NonZero);

        let mut acc = Self::entity_to_subpaths(target_entity);
        if acc.is_empty() {
            return Err(PathOpError::Empty);
        }

        for &id in &ordered_ids[1..] {
            let next_entity = sketch.entities.get(id).ok_or(PathOpError::Empty)?;
            let next_subs = Self::entity_to_subpaths(next_entity);
            if next_subs.is_empty() {
                continue;
            }
            acc = boolean(&acc, &next_subs, op, fill_rule, tol)?;
        }

        let new_entity = Entity::Path {
            subpaths: acc,
            is_construction: false,
        };

        let label = match op {
            BoolOp::Union => "Gabung Path (Union)",
            BoolOp::Difference => "Potong Path (Difference)",
            BoolOp::Intersection => "Iris Path (Intersection)",
            BoolOp::Xor => "Kecualikan Path (Xor)",
        };

        let cmd = ReplaceEntities::new(label, ordered_ids, vec![new_entity])
            .with_styles(vec![target_style])
            .with_layers(vec![target_layer]);

        Ok(cmd)
    }

    /// Gabungkan seluruh entitas terpilih (Union).
    pub fn union(sketch: &Sketch, selected_ids: &[EntityId], tol: f64) -> Result<ReplaceEntities, PathOpError> {
        Self::build_boolean_command(sketch, selected_ids, BoolOp::Union, tol)
    }

    /// Potong entitas paling bawah dengan entitas di atasnya (Difference).
    pub fn difference(sketch: &Sketch, selected_ids: &[EntityId], tol: f64) -> Result<ReplaceEntities, PathOpError> {
        Self::build_boolean_command(sketch, selected_ids, BoolOp::Difference, tol)
    }

    /// Ambil irisan seluruh entitas terpilih (Intersection).
    pub fn intersection(sketch: &Sketch, selected_ids: &[EntityId], tol: f64) -> Result<ReplaceEntities, PathOpError> {
        Self::build_boolean_command(sketch, selected_ids, BoolOp::Intersection, tol)
    }

    /// Ambil bagian yang tidak saling tumpang tindih (Xor).
    pub fn xor(sketch: &Sketch, selected_ids: &[EntityId], tol: f64) -> Result<ReplaceEntities, PathOpError> {
        Self::build_boolean_command(sketch, selected_ids, BoolOp::Xor, tol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::Command;
    use ducad_sketch::path_edit::shape_to_path_rect;
    use ducad_sketch::style::{Paint, Rgba, Style};
    use glam::DVec2;

    #[test]
    fn union_two_rects_inherits_bottom_style() {
        let mut sketch = Sketch::default();

        let s1 = shape_to_path_rect(DVec2::new(0.0, 0.0), DVec2::new(10.0, 10.0));
        let ent1 = Entity::Path {
            subpaths: vec![s1],
            is_construction: false,
        };
        let id1 = sketch.entities.insert(ent1);
        let style1 = Style {
            fill: Some(Paint::Solid(Rgba([1.0, 0.0, 0.0, 1.0]))),
            ..Style::default()
        };
        sketch.styles.insert(id1, style1.clone());

        let s2 = shape_to_path_rect(DVec2::new(5.0, 0.0), DVec2::new(15.0, 10.0));
        let ent2 = Entity::Path {
            subpaths: vec![s2],
            is_construction: false,
        };
        let id2 = sketch.entities.insert(ent2);
        let style2 = Style {
            fill: Some(Paint::Solid(Rgba([0.0, 1.0, 0.0, 1.0]))),
            ..Style::default()
        };
        sketch.styles.insert(id2, style2.clone());

        // id1 dibuat lebih dulu, jadi berada di bawah id2
        let mut cmd = ShapeBuilder::union(&sketch, &[id1, id2], 0.01).expect("union should succeed");
        cmd.apply(&mut sketch);

        assert_eq!(sketch.entities.len(), 1);
        let (new_id, new_ent) = sketch.entities.iter().next().unwrap();
        assert!(matches!(new_ent, Entity::Path { .. }));

        // Style yang diwarisi harus milik objek bawah (style1)
        assert_eq!(sketch.styles.get(new_id), Some(&style1));

        // Revert (undo) harus mengembalikan kedua entitas semula beserta style-nya
        cmd.revert(&mut sketch);
        assert_eq!(sketch.entities.len(), 2);
        let restored_styles: Vec<_> = sketch.styles.values().collect();
        assert_eq!(restored_styles.len(), 2);
        assert!(restored_styles.contains(&&style1));
        assert!(restored_styles.contains(&&style2));
    }

    #[test]
    fn difference_upper_cuts_lower() {
        let mut sketch = Sketch::default();

        let s1 = shape_to_path_rect(DVec2::new(0.0, 0.0), DVec2::new(20.0, 20.0));
        let ent1 = Entity::Path {
            subpaths: vec![s1],
            is_construction: false,
        };
        let id1 = sketch.entities.insert(ent1);

        let s2 = shape_to_path_rect(DVec2::new(5.0, 5.0), DVec2::new(15.0, 15.0));
        let ent2 = Entity::Path {
            subpaths: vec![s2],
            is_construction: false,
        };
        let id2 = sketch.entities.insert(ent2);

        let mut cmd = ShapeBuilder::difference(&sketch, &[id1, id2], 0.01).expect("difference should succeed");
        cmd.apply(&mut sketch);

        assert_eq!(sketch.entities.len(), 1);
        let (_, new_ent) = sketch.entities.iter().next().unwrap();
        if let Entity::Path { subpaths, .. } = new_ent {
            // Hasil pemotongan bujur sangkar di tengah menghasilkan 2 subpath (outer + hole)
            assert_eq!(subpaths.len(), 2);
        } else {
            panic!("Expected Entity::Path");
        }
    }

    #[test]
    fn intersection_disjoint_returns_error() {
        let mut sketch = Sketch::default();

        let s1 = shape_to_path_rect(DVec2::new(0.0, 0.0), DVec2::new(10.0, 10.0));
        let id1 = sketch.entities.insert(Entity::Path {
            subpaths: vec![s1],
            is_construction: false,
        });

        let s2 = shape_to_path_rect(DVec2::new(20.0, 20.0), DVec2::new(30.0, 30.0));
        let id2 = sketch.entities.insert(Entity::Path {
            subpaths: vec![s2],
            is_construction: false,
        });

        let res = ShapeBuilder::intersection(&sketch, &[id1, id2], 0.01);
        assert_eq!(res.err(), Some(PathOpError::Empty));
    }
}
