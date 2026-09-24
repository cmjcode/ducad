//! Precision Transform dan Group selection logic (M2.5).

use ducad_sketch::commands::ReplaceEntities;
use ducad_sketch::entity::EntityId;
use ducad_sketch::layer::GroupId;
use ducad_sketch::kurbo;
use ducad_sketch::path_edit::transform_entity;
use ducad_sketch::Sketch;
use glam::DVec2;

/// Titik tumpu transformasi (9 titik anchor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PivotAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    #[default]
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl PivotAnchor {
    /// Menghitung koordinat titik tumpu dari batas kotak pembatas (bbox).
    pub fn point_from_bbox(self, min: DVec2, max: DVec2) -> DVec2 {
        let mid = (min + max) * 0.5;
        match self {
            Self::TopLeft => DVec2::new(min.x, max.y),
            Self::TopCenter => DVec2::new(mid.x, max.y),
            Self::TopRight => DVec2::new(max.x, max.y),
            Self::CenterLeft => DVec2::new(min.x, mid.y),
            Self::Center => mid,
            Self::CenterRight => DVec2::new(max.x, mid.y),
            Self::BottomLeft => DVec2::new(min.x, min.y),
            Self::BottomCenter => DVec2::new(mid.x, min.y),
            Self::BottomRight => DVec2::new(max.x, min.y),
        }
    }
}

/// Parameter transformasi presisi.
#[derive(Debug, Clone, Default)]
pub struct TransformParams {
    pub target_x: Option<f64>,
    pub target_y: Option<f64>,
    pub target_w: Option<f64>,
    pub target_h: Option<f64>,
    pub rotation_rad: Option<f64>,
    pub scale_pct_x: Option<f64>,
    pub scale_pct_y: Option<f64>,
    pub lock_aspect_ratio: bool,
    pub pivot: PivotAnchor,
}

pub struct PrecisionTransform;

impl PrecisionTransform {
    /// Menghitung matriks transformasi `kurbo::Affine` dari parameter yang diberikan.
    pub fn compute_affine(
        bbox_min: DVec2,
        bbox_max: DVec2,
        params: &TransformParams,
    ) -> kurbo::Affine {
        let pivot_pt = params.pivot.point_from_bbox(bbox_min, bbox_max);
        let orig_w = (bbox_max.x - bbox_min.x).max(1e-6);
        let orig_h = (bbox_max.y - bbox_min.y).max(1e-6);

        // Skala
        let (mut sx, mut sy) = if let (Some(w), Some(h)) = (params.target_w, params.target_h) {
            (w / orig_w, h / orig_h)
        } else if let Some(w) = params.target_w {
            let s = w / orig_w;
            if params.lock_aspect_ratio {
                (s, s)
            } else {
                (s, 1.0)
            }
        } else if let Some(h) = params.target_h {
            let s = h / orig_h;
            if params.lock_aspect_ratio {
                (s, s)
            } else {
                (1.0, s)
            }
        } else if let (Some(px), Some(py)) = (params.scale_pct_x, params.scale_pct_y) {
            (px / 100.0, py / 100.0)
        } else if let Some(px) = params.scale_pct_x {
            let s = px / 100.0;
            if params.lock_aspect_ratio {
                (s, s)
            } else {
                (s, 1.0)
            }
        } else if let Some(py) = params.scale_pct_y {
            let s = py / 100.0;
            if params.lock_aspect_ratio {
                (s, s)
            } else {
                (1.0, s)
            }
        } else {
            (1.0, 1.0)
        };

        if params.lock_aspect_ratio && (sx - sy).abs() > 1e-9 {
            let avg = (sx + sy) * 0.5;
            sx = avg;
            sy = avg;
        }

        // Rotasi
        let rot = params.rotation_rad.unwrap_or(0.0);

        // Translasi posisi pivot
        let tx = params.target_x.map(|x| x - pivot_pt.x).unwrap_or(0.0);
        let ty = params.target_y.map(|y| y - pivot_pt.y).unwrap_or(0.0);

        kurbo::Affine::translate((pivot_pt.x + tx, pivot_pt.y + ty))
            * kurbo::Affine::rotate(rot)
            * kurbo::Affine::scale_non_uniform(sx, sy)
            * kurbo::Affine::translate((-pivot_pt.x, -pivot_pt.y))
    }

    /// Menghasilkan command `ReplaceEntities` untuk mentransformasikan entitas terpilih.
    pub fn build_transform_command(
        sketch: &Sketch,
        selected_ids: &[EntityId],
        params: &TransformParams,
    ) -> Option<ReplaceEntities> {
        if selected_ids.is_empty() {
            return None;
        }

        let mut bmin = DVec2::splat(f64::INFINITY);
        let mut bmax = DVec2::splat(f64::NEG_INFINITY);
        let mut found = false;

        for &id in selected_ids {
            if let Some(e) = sketch.entities.get(id) {
                if let Some((emin, emax)) = e.bounding_box() {
                    bmin = bmin.min(emin);
                    bmax = bmax.max(emax);
                    found = true;
                }
            }
        }

        if !found {
            return None;
        }

        let affine = Self::compute_affine(bmin, bmax, params);

        let mut new_entities = Vec::with_capacity(selected_ids.len());
        let mut styles = Vec::with_capacity(selected_ids.len());
        let mut layers = Vec::with_capacity(selected_ids.len());

        for &id in selected_ids {
            let ent = sketch.entities.get(id)?;
            let transformed = transform_entity(ent, affine);
            new_entities.push(transformed);
            styles.push(sketch.styles.get(id).cloned());
            layers.push(sketch.entity_layer.get(id).copied());
        }

        let cmd = ReplaceEntities::new("Transform Presisi", selected_ids.to_vec(), new_entities)
            .with_styles(styles)
            .with_layers(layers);

        Some(cmd)
    }
}

/// Mencari root group (grup terluar) dari hierarki group.
pub fn find_root_group(sketch: &Sketch, mut gid: GroupId) -> GroupId {
    while let Some(grp) = sketch.groups.get(gid) {
        if let Some(parent_gid) = grp.parent {
            gid = parent_gid;
        } else {
            break;
        }
    }
    gid
}

/// Mengambil seluruh EntityId anggota grup beserta subgrupnya secara rekursif.
pub fn get_group_members_recursive(sketch: &Sketch, gid: GroupId) -> Vec<EntityId> {
    let mut result = Vec::new();
    let mut group_stack = vec![gid];
    while let Some(current_gid) = group_stack.pop() {
        if let Some(grp) = sketch.groups.get(current_gid) {
            result.extend(grp.members.iter().copied());
        }
        for (child_gid, child_grp) in &sketch.groups {
            if child_grp.parent == Some(current_gid) {
                group_stack.push(child_gid);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    result.retain(|id| seen.insert(*id));
    result
}

/// Menentukan daftar entitas yang terpilih saat user mengklik sebuah entitas:
/// - Jika tidak sedang dalam mode isolasi: memilih seluruh anggota grup terluar (root group).
/// - Jika sedang dalam mode isolasi grup G:
///   - Jika objek berada dalam subgrup di bawah G, memilih subgrup tersebut.
///   - Jika objek berada langsung pada level G, memilih objek itu sendiri.
pub fn resolve_group_selection(
    sketch: &Sketch,
    clicked_id: EntityId,
    isolated_group: Option<GroupId>,
) -> Vec<EntityId> {
    if let Some(&direct_gid) = sketch.entity_group.get(clicked_id) {
        if let Some(iso_gid) = isolated_group {
            let mut chain = Vec::new();
            let mut curr = direct_gid;
            chain.push(curr);
            while let Some(grp) = sketch.groups.get(curr) {
                if let Some(p) = grp.parent {
                    chain.push(p);
                    curr = p;
                } else {
                    break;
                }
            }

            if let Some(pos) = chain.iter().position(|&g| g == iso_gid) {
                if pos > 0 {
                    let target_subgroup = chain[pos - 1];
                    get_group_members_recursive(sketch, target_subgroup)
                } else {
                    vec![clicked_id]
                }
            } else {
                let root_gid = find_root_group(sketch, direct_gid);
                get_group_members_recursive(sketch, root_gid)
            }
        } else {
            let root_gid = find_root_group(sketch, direct_gid);
            get_group_members_recursive(sketch, root_gid)
        }
    } else {
        vec![clicked_id]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::Command;
    use ducad_sketch::commands::GroupEntities;
    use ducad_sketch::entity::Entity;
    use ducad_sketch::path_edit::shape_to_path_rect;

    #[test]
    fn group_click_selects_top_group() {
        let mut sketch = Sketch::default();
        let r1 = shape_to_path_rect(DVec2::new(0.0, 0.0), DVec2::new(10.0, 10.0));
        let id1 = sketch.entities.insert(Entity::Path {
            subpaths: vec![r1],
            is_construction: false,
        });

        let r2 = shape_to_path_rect(DVec2::new(20.0, 0.0), DVec2::new(30.0, 10.0));
        let id2 = sketch.entities.insert(Entity::Path {
            subpaths: vec![r2],
            is_construction: false,
        });

        let r3 = shape_to_path_rect(DVec2::new(40.0, 0.0), DVec2::new(50.0, 10.0));
        let id3 = sketch.entities.insert(Entity::Path {
            subpaths: vec![r3],
            is_construction: false,
        });

        // Buat Grup 1 (inner): [id1, id2]
        let mut cmd1 = GroupEntities::new(vec![id1, id2], "InnerGroup");
        cmd1.apply(&mut sketch);
        let g_inner = cmd1.created_group_id().unwrap();

        // Buat Grup 2 (outer/top): [id3] dan jadikan g_inner anak dari g_outer
        let mut cmd2 = GroupEntities::new(vec![id3], "OuterGroup");
        cmd2.apply(&mut sketch);
        let g_outer = cmd2.created_group_id().unwrap();
        sketch.groups.get_mut(g_inner).unwrap().parent = Some(g_outer);

        // 1. Tanpa isolasi: klik id1 harus memilih seluruh grup terluar (id1, id2, id3)
        let sel_top = resolve_group_selection(&sketch, id1, None);
        assert_eq!(sel_top.len(), 3);
        assert!(sel_top.contains(&id1));
        assert!(sel_top.contains(&id2));
        assert!(sel_top.contains(&id3));

        // 2. Isolasi g_outer: klik id1 harus memilih subgrup g_inner (id1, id2)
        let sel_isolated_outer = resolve_group_selection(&sketch, id1, Some(g_outer));
        assert_eq!(sel_isolated_outer.len(), 2);
        assert!(sel_isolated_outer.contains(&id1));
        assert!(sel_isolated_outer.contains(&id2));
        assert!(!sel_isolated_outer.contains(&id3));

        // 3. Isolasi g_inner: klik id1 harus memilih id1 saja
        let sel_isolated_inner = resolve_group_selection(&sketch, id1, Some(g_inner));
        assert_eq!(sel_isolated_inner, vec![id1]);
    }
}
