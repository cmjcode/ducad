//! Tool Node Edit interaktif (M2.3).
//!
//! Logika murni hit-testing, manipulasi entitas path vektor, dan rendering
//! overlay node serta handle Bézier.

use ducad_render::LineVertex;
use ducad_sketch::entity::{Entity, Subpath};
use ducad_sketch::path_edit::{
    break_at_node, closest_point, delete_node, get_node_handles, insert_node_at, move_handle,
    move_node, node_kind, reverse, seg_to_curve, seg_to_line, set_node_kind, HandleSide, NodeKind,
};
use glam::DVec2;

/// Handler murni untuk perkakas edit node Bézier.
pub struct NodeEditTool;

impl NodeEditTool {
    /// Hit-test node kontrol pada subpath.
    pub fn hit_test_node(sub: &Subpath, cursor: DVec2, hit_radius: f64) -> Option<usize> {
        let mut best = None;
        let mut min_dist = hit_radius;

        for (i, p) in sub.nodes().enumerate() {
            let d = (p - cursor).length();
            if d <= min_dist {
                min_dist = d;
                best = Some(i);
            }
        }
        best
    }

    /// Hit-test handle kontrol (arah masuk atau keluar) pada subpath.
    pub fn hit_test_handle(
        sub: &Subpath,
        cursor: DVec2,
        hit_radius: f64,
    ) -> Option<(usize, HandleSide)> {
        let count = sub.node_count();
        let mut best = None;
        let mut min_dist = hit_radius;

        for i in 0..count {
            let (h_in, h_out) = get_node_handles(sub, i);
            if let Some(hi) = h_in {
                let d = (hi - cursor).length();
                if d <= min_dist {
                    min_dist = d;
                    best = Some((i, HandleSide::In));
                }
            }
            if let Some(ho) = h_out {
                let d = (ho - cursor).length();
                if d <= min_dist {
                    min_dist = d;
                    best = Some((i, HandleSide::Out));
                }
            }
        }
        best
    }

    /// Hit-test segmen kurva/garis pada subpath.
    /// Mengembalikan indeks segmen dan nilai parameter `t` jika berada dalam radius toleransi.
    pub fn hit_test_segment(sub: &Subpath, cursor: DVec2, hit_radius: f64) -> Option<(usize, f64)> {
        let (seg, t, _, dist) = closest_point(sub, cursor);
        if dist <= hit_radius {
            Some((seg, t))
        } else {
            None
        }
    }

    /// Menggeser node pada entitas `Entity::Path`.
    pub fn move_node_on_entity(
        entity: &Entity,
        sub_idx: usize,
        node_idx: usize,
        to: DVec2,
    ) -> Option<Entity> {
        let Entity::Path { subpaths, is_construction } = entity else {
            return None;
        };
        if sub_idx >= subpaths.len() {
            return None;
        }
        let mut new_subs = subpaths.clone();
        new_subs[sub_idx] = move_node(&new_subs[sub_idx], node_idx, to);
        Some(Entity::Path {
            subpaths: new_subs,
            is_construction: *is_construction,
        })
    }

    /// Menggeser handle Bézier pada entitas `Entity::Path`.
    pub fn move_handle_on_entity(
        entity: &Entity,
        sub_idx: usize,
        node_idx: usize,
        side: HandleSide,
        to: DVec2,
        keep_smooth: bool,
    ) -> Option<Entity> {
        let Entity::Path { subpaths, is_construction } = entity else {
            return None;
        };
        if sub_idx >= subpaths.len() {
            return None;
        }
        let mut new_subs = subpaths.clone();
        new_subs[sub_idx] = move_handle(&new_subs[sub_idx], node_idx, side, to, keep_smooth);
        Some(Entity::Path {
            subpaths: new_subs,
            is_construction: *is_construction,
        })
    }

    /// Menyisipkan node baru pada entitas `Entity::Path` (de Casteljau).
    pub fn insert_node_on_entity(
        entity: &Entity,
        sub_idx: usize,
        seg_idx: usize,
        t: f64,
    ) -> Option<Entity> {
        let Entity::Path { subpaths, is_construction } = entity else {
            return None;
        };
        if sub_idx >= subpaths.len() {
            return None;
        }
        let mut new_subs = subpaths.clone();
        new_subs[sub_idx] = insert_node_at(&new_subs[sub_idx], seg_idx, t);
        Some(Entity::Path {
            subpaths: new_subs,
            is_construction: *is_construction,
        })
    }

    /// Menghapus node pada entitas `Entity::Path`.
    pub fn delete_node_on_entity(
        entity: &Entity,
        sub_idx: usize,
        node_idx: usize,
    ) -> Option<Entity> {
        let Entity::Path { subpaths, is_construction } = entity else {
            return None;
        };
        if sub_idx >= subpaths.len() {
            return None;
        }
        let mut new_subs = subpaths.clone();
        let updated = delete_node(&new_subs[sub_idx], node_idx)?;
        new_subs[sub_idx] = updated;
        Some(Entity::Path {
            subpaths: new_subs,
            is_construction: *is_construction,
        })
    }

    /// Menyetel jenis node (Corner, Smooth, Symmetric) pada entitas `Entity::Path`.
    pub fn set_node_kind_on_entity(
        entity: &Entity,
        sub_idx: usize,
        node_idx: usize,
        kind: NodeKind,
    ) -> Option<Entity> {
        let Entity::Path { subpaths, is_construction } = entity else {
            return None;
        };
        if sub_idx >= subpaths.len() {
            return None;
        }
        let mut new_subs = subpaths.clone();
        new_subs[sub_idx] = set_node_kind(&new_subs[sub_idx], node_idx, kind);
        Some(Entity::Path {
            subpaths: new_subs,
            is_construction: *is_construction,
        })
    }

    /// Mengubah segmen menjadi garis lurus.
    pub fn seg_to_line_on_entity(
        entity: &Entity,
        sub_idx: usize,
        seg_idx: usize,
    ) -> Option<Entity> {
        let Entity::Path { subpaths, is_construction } = entity else {
            return None;
        };
        if sub_idx >= subpaths.len() {
            return None;
        }
        let mut new_subs = subpaths.clone();
        new_subs[sub_idx] = seg_to_line(&new_subs[sub_idx], seg_idx);
        Some(Entity::Path {
            subpaths: new_subs,
            is_construction: *is_construction,
        })
    }

    /// Mengubah segmen menjadi kurva Bézier kubik.
    pub fn seg_to_curve_on_entity(
        entity: &Entity,
        sub_idx: usize,
        seg_idx: usize,
    ) -> Option<Entity> {
        let Entity::Path { subpaths, is_construction } = entity else {
            return None;
        };
        if sub_idx >= subpaths.len() {
            return None;
        }
        let mut new_subs = subpaths.clone();
        new_subs[sub_idx] = seg_to_curve(&new_subs[sub_idx], seg_idx);
        Some(Entity::Path {
            subpaths: new_subs,
            is_construction: *is_construction,
        })
    }

    /// Memutus subpath pada node ke-`node_idx`.
    pub fn break_at_node_on_entity(
        entity: &Entity,
        sub_idx: usize,
        node_idx: usize,
    ) -> Option<Entity> {
        let Entity::Path { subpaths, is_construction } = entity else {
            return None;
        };
        if sub_idx >= subpaths.len() {
            return None;
        }
        let (a, b) = break_at_node(&subpaths[sub_idx], node_idx);
        let mut new_subs = Vec::new();
        for (idx, sub) in subpaths.iter().enumerate() {
            if idx == sub_idx {
                new_subs.push(a.clone());
                new_subs.push(b.clone());
            } else {
                new_subs.push(sub.clone());
            }
        }
        Some(Entity::Path {
            subpaths: new_subs,
            is_construction: *is_construction,
        })
    }

    /// Membalik urutan arah subpath.
    pub fn reverse_on_entity(entity: &Entity, sub_idx: usize) -> Option<Entity> {
        let Entity::Path { subpaths, is_construction } = entity else {
            return None;
        };
        if sub_idx >= subpaths.len() {
            return None;
        }
        let mut new_subs = subpaths.clone();
        new_subs[sub_idx] = reverse(&new_subs[sub_idx]);
        Some(Entity::Path {
            subpaths: new_subs,
            is_construction: *is_construction,
        })
    }

    /// Menghasilkan baris simpul [`LineVertex`] untuk merender overlay visual node dan handle Bézier.
    pub fn overlay_lines(
        sub: &Subpath,
        selected_nodes: &[usize],
        marker_size: f64,
        color_node: [f32; 4],
        color_selected: [f32; 4],
        color_handle: [f32; 4],
    ) -> Vec<LineVertex> {
        let mut lines = Vec::new();
        let half = marker_size * 0.5;

        for (i, p) in sub.nodes().enumerate() {
            let is_selected = selected_nodes.contains(&i);
            let col = if is_selected { color_selected } else { color_node };
            let kind = node_kind(sub, i);

            // Render node marker
            match kind {
                NodeKind::Corner => {
                    // Persegi untuk node sudut
                    let p0 = [p.x - half, p.y - half];
                    let p1 = [p.x + half, p.y - half];
                    let p2 = [p.x + half, p.y + half];
                    let p3 = [p.x - half, p.y + half];
                    add_line(&mut lines, p0, p1, col);
                    add_line(&mut lines, p1, p2, col);
                    add_line(&mut lines, p2, p3, col);
                    add_line(&mut lines, p3, p0, col);
                }
                NodeKind::Smooth | NodeKind::Symmetric => {
                    // Belah ketupat untuk node halus / simetris
                    let p_top = [p.x, p.y + half];
                    let p_right = [p.x + half, p.y];
                    let p_bot = [p.x, p.y - half];
                    let p_left = [p.x - half, p.y];
                    add_line(&mut lines, p_top, p_right, col);
                    add_line(&mut lines, p_right, p_bot, col);
                    add_line(&mut lines, p_bot, p_left, col);
                    add_line(&mut lines, p_left, p_top, col);
                }
            }

            // Jika node terpilih, tampilkan garis lengan dan titik handle kontrol
            if is_selected {
                let (h_in, h_out) = get_node_handles(sub, i);
                if let Some(hi) = h_in {
                    add_line(&mut lines, [p.x, p.y], [hi.x, hi.y], color_handle);
                    add_handle_marker(&mut lines, hi, half * 0.75, color_handle);
                }
                if let Some(ho) = h_out {
                    add_line(&mut lines, [p.x, p.y], [ho.x, ho.y], color_handle);
                    add_handle_marker(&mut lines, ho, half * 0.75, color_handle);
                }
            }
        }

        lines
    }
}

fn add_line(lines: &mut Vec<LineVertex>, a: [f64; 2], b: [f64; 2], color: [f32; 4]) {
    lines.push(LineVertex {
        position: [a[0] as f32, a[1] as f32, 0.0],
        color,
    });
    lines.push(LineVertex {
        position: [b[0] as f32, b[1] as f32, 0.0],
        color,
    });
}

fn add_handle_marker(lines: &mut Vec<LineVertex>, p: DVec2, radius: f64, color: [f32; 4]) {
    let p0 = [p.x - radius, p.y - radius];
    let p1 = [p.x + radius, p.y - radius];
    let p2 = [p.x + radius, p.y + radius];
    let p3 = [p.x - radius, p.y + radius];
    add_line(lines, p0, p1, color);
    add_line(lines, p1, p2, color);
    add_line(lines, p2, p3, color);
    add_line(lines, p3, p0, color);
}

use ducad_sketch::constraint::{point_ref_position, Constraint, PointRef};
use ducad_sketch::entity::EntityId;
use ducad_sketch::sketch::Sketch;
use ducad_ui::ConstraintAction;

/// Menghitung constraint yang valid berdasarkan jumlah node yang dipilih.
pub fn valid_constraints_for_node_count(count: usize) -> Vec<ConstraintAction> {
    match count {
        1 => vec![ConstraintAction::ApplyFixed],
        2 => vec![
            ConstraintAction::ApplyCoincident,
            ConstraintAction::ApplyFixed,
            ConstraintAction::ApplyHorizontal,
            ConstraintAction::ApplyVertical,
            ConstraintAction::ApplyDistance,
            ConstraintAction::ApplySymmetric,
        ],
        _ => Vec::new(),
    }
}

/// Membangun constraint dari sekumpulan node path terpilih dan aksi yang diminta.
pub fn build_node_constraint(
    sketch: &Sketch,
    nodes: &[(EntityId, u16, u32)],
    action: ConstraintAction,
) -> Option<Constraint> {
    match action {
        ConstraintAction::ApplyFixed => {
            let (id, sub, node) = nodes.first().copied()?;
            let pr = PointRef::PathNode { id, sub, node };
            let pos = point_ref_position(sketch, &pr)?;
            Some(Constraint::Fixed { point: pr, target: pos })
        }
        ConstraintAction::ApplyCoincident => {
            if nodes.len() < 2 {
                return None;
            }
            let a = PointRef::PathNode { id: nodes[0].0, sub: nodes[0].1, node: nodes[0].2 };
            let b = PointRef::PathNode { id: nodes[1].0, sub: nodes[1].1, node: nodes[1].2 };
            Some(Constraint::Coincident { a, b })
        }
        ConstraintAction::ApplyHorizontal => {
            if nodes.len() < 2 {
                return None;
            }
            let a = PointRef::PathNode { id: nodes[0].0, sub: nodes[0].1, node: nodes[0].2 };
            let b = PointRef::PathNode { id: nodes[1].0, sub: nodes[1].1, node: nodes[1].2 };
            Some(Constraint::HorizontalPoints { a, b })
        }
        ConstraintAction::ApplyVertical => {
            if nodes.len() < 2 {
                return None;
            }
            let a = PointRef::PathNode { id: nodes[0].0, sub: nodes[0].1, node: nodes[0].2 };
            let b = PointRef::PathNode { id: nodes[1].0, sub: nodes[1].1, node: nodes[1].2 };
            Some(Constraint::VerticalPoints { a, b })
        }
        ConstraintAction::ApplyDistance => {
            if nodes.len() < 2 {
                return None;
            }
            let a = PointRef::PathNode { id: nodes[0].0, sub: nodes[0].1, node: nodes[0].2 };
            let b = PointRef::PathNode { id: nodes[1].0, sub: nodes[1].1, node: nodes[1].2 };
            let pa = point_ref_position(sketch, &a)?;
            let pb = point_ref_position(sketch, &b)?;
            let dist = (pb - pa).length();
            Some(Constraint::Distance { a, b, value: dist })
        }
        ConstraintAction::ApplySymmetric => {
            if nodes.len() < 2 {
                return None;
            }
            let a = PointRef::PathNode { id: nodes[0].0, sub: nodes[0].1, node: nodes[0].2 };
            let b = PointRef::PathNode { id: nodes[1].0, sub: nodes[1].1, node: nodes[1].2 };
            let axis = sketch.entities.iter().find_map(|(id, e)| match e {
                Entity::Line { is_construction: true, .. } => Some(id),
                _ => None,
            }).or_else(|| {
                sketch.entities.iter().find_map(|(id, e)| match e {
                    Entity::Line { .. } => Some(id),
                    _ => None,
                })
            })?;
            Some(Constraint::Symmetric { a, b, axis })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use ducad_sketch::entity::PathSeg;

    #[test]
    fn test_hit_test_node_and_handle() {
        let sub = Subpath {
            start: DVec2::new(10.0, 20.0),
            segs: vec![
                PathSeg::Cubic {
                    c1: DVec2::new(15.0, 25.0),
                    c2: DVec2::new(25.0, 25.0),
                    end: DVec2::new(30.0, 20.0),
                },
            ],
            closed: false,
        };

        // Node 0 ada di (10, 20)
        assert_eq!(NodeEditTool::hit_test_node(&sub, DVec2::new(10.2, 20.1), 0.5), Some(0));
        assert_eq!(NodeEditTool::hit_test_node(&sub, DVec2::new(15.0, 20.0), 0.5), None);

        // Handle Out dari Node 0 ada di c1 = (15, 25)
        assert_eq!(
            NodeEditTool::hit_test_handle(&sub, DVec2::new(15.1, 24.9), 0.5),
            Some((0, HandleSide::Out))
        );

        // Handle In dari Node 1 ada di c2 = (25, 25)
        assert_eq!(
            NodeEditTool::hit_test_handle(&sub, DVec2::new(24.9, 25.1), 0.5),
            Some((1, HandleSide::In))
        );
    }

    #[test]
    fn test_insert_and_delete_node_on_entity() {
        let ent = Entity::Path {
            subpaths: vec![Subpath {
                start: DVec2::new(0.0, 0.0),
                segs: vec![PathSeg::Line { end: DVec2::new(100.0, 0.0) }],
                closed: false,
            }],
            is_construction: false,
        };

        let split_ent = NodeEditTool::insert_node_on_entity(&ent, 0, 0, 0.5).expect("insert should succeed");
        if let Entity::Path { subpaths, .. } = &split_ent {
            assert_eq!(subpaths[0].segs.len(), 2);
            assert_eq!(subpaths[0].node(1), Some(DVec2::new(50.0, 0.0)));
        } else {
            panic!("Expected Entity::Path");
        }

        let del_ent = NodeEditTool::delete_node_on_entity(&split_ent, 0, 1).expect("delete should succeed");
        if let Entity::Path { subpaths, .. } = &del_ent {
            assert_eq!(subpaths[0].segs.len(), 1);
            assert_eq!(subpaths[0].node(1), Some(DVec2::new(100.0, 0.0)));
        } else {
            panic!("Expected Entity::Path");
        }
    }

    #[test]
    fn test_overlay_lines_non_empty() {
        let sub = Subpath {
            start: DVec2::new(0.0, 0.0),
            segs: vec![PathSeg::Line { end: DVec2::new(10.0, 0.0) }],
            closed: false,
        };
        let lines = NodeEditTool::overlay_lines(
            &sub,
            &[0],
            1.0,
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
        );
        assert!(!lines.is_empty(), "Overlay lines should not be empty");
    }

    #[test]
    fn test_valid_constraints_and_build_node_constraint() {
        assert_eq!(valid_constraints_for_node_count(0), vec![]);
        assert_eq!(valid_constraints_for_node_count(1), vec![ConstraintAction::ApplyFixed]);
        let c2 = valid_constraints_for_node_count(2);
        assert!(c2.contains(&ConstraintAction::ApplyCoincident));
        assert!(c2.contains(&ConstraintAction::ApplyDistance));
        assert!(c2.contains(&ConstraintAction::ApplyHorizontal));
        assert!(c2.contains(&ConstraintAction::ApplyVertical));

        let mut sketch = Sketch::default();
        let p = sketch.entities.insert(Entity::Path {
            subpaths: vec![Subpath {
                start: DVec2::new(10.0, 20.0),
                segs: vec![PathSeg::Line { end: DVec2::new(30.0, 20.0) }],
                closed: false,
            }],
            is_construction: false,
        });

        let n0 = (p, 0, 0);
        let n1 = (p, 0, 1);

        // Fixed
        let fixed = build_node_constraint(&sketch, &[n0], ConstraintAction::ApplyFixed);
        assert!(matches!(fixed, Some(Constraint::Fixed { .. })));

        // Coincident
        let coinc = build_node_constraint(&sketch, &[n0, n1], ConstraintAction::ApplyCoincident);
        assert!(matches!(coinc, Some(Constraint::Coincident { .. })));

        // Horizontal
        let horiz = build_node_constraint(&sketch, &[n0, n1], ConstraintAction::ApplyHorizontal);
        assert!(matches!(horiz, Some(Constraint::HorizontalPoints { .. })));

        // Vertical
        let vert = build_node_constraint(&sketch, &[n0, n1], ConstraintAction::ApplyVertical);
        assert!(matches!(vert, Some(Constraint::VerticalPoints { .. })));

        // Distance
        let dist = build_node_constraint(&sketch, &[n0, n1], ConstraintAction::ApplyDistance);
        if let Some(Constraint::Distance { value, .. }) = dist {
            assert!((value - 20.0).abs() < 1e-6);
        } else {
            panic!("Expected Constraint::Distance");
        }
    }
}

