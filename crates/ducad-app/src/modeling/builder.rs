use std::collections::HashSet;

use ducad_sketch::{find_closed_regions, ClosedRegion, EntityId, Sketch};
use glam::DVec2;

use crate::app::DuCADApp;

/// Hitung centroid snap untuk region tertutup di sketch.
pub fn region_center_snap(
    sketch: &Sketch,
    exclude: &HashSet<EntityId>,
    target: DVec2,
    tolerance: f64,
) -> Option<DVec2> {
    let mut best: Option<(DVec2, f64)> = None;
    for reg in find_closed_regions(sketch) {
        if !exclude.is_empty() && reg.entity_ids.is_subset(exclude) {
            continue;
        }
        let d = (reg.centroid - target).length();
        if d <= tolerance && best.as_ref().is_none_or(|(_, bd)| d < *bd) {
            best = Some((reg.centroid, d));
        }
    }
    best.map(|(c, _)| c)
}

impl DuCADApp {
    /// Extrude profil pada bidang sketsa aktif sepanjang `distance`.
    pub fn extrude_profile_active_plane(
        &self,
        profile: &ducad_kernel::Profile,
        distance: f64,
    ) -> anyhow::Result<ducad_kernel::KernelShape> {
        let orig = self.active_plane.to_world_f64((0.0, 0.0), 0.0);
        let u_ax = [
            self.active_plane.u_axis.x as f64,
            self.active_plane.u_axis.y as f64,
            self.active_plane.u_axis.z as f64,
        ];
        let v_ax = [
            self.active_plane.v_axis.x as f64,
            self.active_plane.v_axis.y as f64,
            self.active_plane.v_axis.z as f64,
        ];
        let n_ax = [
            self.active_plane.normal.x as f64,
            self.active_plane.normal.y as f64,
            self.active_plane.normal.z as f64,
        ];
        ducad_kernel::extrude_profile_on_plane(profile, orig, u_ax, v_ax, n_ax, distance)
    }

    /// Hitung centroid rata-rata dari profil sketch tertutup yang sedang aktif terpilih.
    pub fn selected_closed_region_centroid(&self) -> Option<DVec2> {
        if self.tool != crate::types::ToolKind::Select || self.selected.is_empty() {
            return None;
        }
        let closed_regions = find_closed_regions(self.sketch());
        let selected_regions: Vec<&ClosedRegion> = closed_regions
            .iter()
            .filter(|r| r.entity_ids.is_subset(&self.selected))
            .collect();
        if !selected_regions.is_empty() {
            let total_area: f64 = selected_regions.iter().map(|r| r.area.max(1e-4)).sum();
            let mut cx = 0.0;
            let mut cy = 0.0;
            for r in &selected_regions {
                cx += r.centroid.x * r.area.max(1e-4);
                cy += r.centroid.y * r.area.max(1e-4);
            }
            return Some(DVec2::new(cx / total_area, cy / total_area));
        }

        // Fallback: hitung centroid dari profile jika entitas terpilih membentuk loop tertutup
        if let Ok(profile) = crate::model::build_profile_from_selection(self.sketch(), &self.selected) {
            match profile {
                ducad_kernel::Profile::Circle { center, .. } | ducad_kernel::Profile::Ellipse { center, .. } => {
                    return Some(DVec2::new(center.0, center.1));
                }
                ducad_kernel::Profile::Loop(segments) => {
                    let mut sum = DVec2::ZERO;
                    let mut count = 0.0;
                    for seg in &segments {
                        match seg {
                            ducad_kernel::ProfileSegment::Line { start, end } => {
                                sum += DVec2::new(start.0, start.1) + DVec2::new(end.0, end.1);
                                count += 2.0;
                            }
                            ducad_kernel::ProfileSegment::Arc { start, via, end } => {
                                sum += DVec2::new(start.0, start.1) + DVec2::new(via.0, via.1) + DVec2::new(end.0, end.1);
                                count += 3.0;
                            }
                        }
                    }
                    if count > 0.0 {
                        return Some(sum / count);
                    }
                }
            }
        }
        None
    }

    /// Grup-grup entitas yang MASING-MASING dapat titik "+" SENDIRI di sketch aktif.
    pub fn sketch_move_groups(&self) -> Vec<HashSet<EntityId>> {
        if !self.selected.is_empty() {
            return vec![self.selected.clone()];
        }
        find_closed_regions(self.sketch())
            .into_iter()
            .map(|r| r.entity_ids)
            .collect()
    }

    /// Titik pusat satu grup `ids` dari `sketch_move_groups`.
    pub fn group_centroid(&self, ids: &HashSet<EntityId>) -> Option<DVec2> {
        let mut sum = DVec2::ZERO;
        let mut count = 0usize;
        for id in ids {
            if let Some(e) = self.sketch().entities.get(*id) {
                sum += e.midpoint().or_else(|| e.center()).unwrap_or(DVec2::ZERO);
                count += 1;
            }
        }
        (count > 0).then(|| sum / count as f64)
    }

    /// Jangkar grup yang SEDANG jadi target gizmo geser sketch.
    pub fn sketch_move_anchor(&self) -> Option<glam::Vec3> {
        if !self.selected_bodies.is_empty() {
            return None;
        }
        let ids = self.sketch_move_target.as_ref()?;
        let centroid = self.group_centroid(ids)?;
        Some(self.active_plane.to_world(centroid, 0.05))
    }

    /// Titik tengah region tertutup LAIN di sketch aktif yang paling dekat dengan `target`.
    pub fn find_region_center_snap(
        &self,
        exclude: &HashSet<EntityId>,
        target: DVec2,
        tolerance: f64,
    ) -> Option<DVec2> {
        region_center_snap(self.sketch(), exclude, target, tolerance)
    }

    /// Target nudge KEYBOARD (Cmd+Panah atau armed+panah-polos).
    pub fn nudge_target_ids(&self) -> Option<Vec<EntityId>> {
        if !self.selected.is_empty() {
            return Some(self.selected.iter().copied().collect());
        }
        if let Some(ids) = &self.sketch_move_target {
            return Some(ids.iter().copied().collect());
        }
        let regions = find_closed_regions(self.sketch());
        if regions.len() == 1 {
            return Some(regions[0].entity_ids.iter().copied().collect());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::Entity;

    #[test]
    fn snaps_to_other_closed_region_centroid() {
        let mut sketch = Sketch::default();
        let a0 = sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
        ));
        let a1 = sketch.entities.insert(Entity::line(
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 5.0),
        ));
        let a2 = sketch.entities.insert(Entity::line(
            DVec2::new(10.0, 5.0),
            DVec2::new(0.0, 5.0),
        ));
        let a3 = sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 5.0),
            DVec2::new(0.0, 0.0),
        ));
        sketch.entities.insert(Entity::line(
            DVec2::new(20.0, 0.0),
            DVec2::new(30.0, 0.0),
        ));
        sketch.entities.insert(Entity::line(
            DVec2::new(30.0, 0.0),
            DVec2::new(30.0, 5.0),
        ));
        sketch.entities.insert(Entity::line(
            DVec2::new(30.0, 5.0),
            DVec2::new(20.0, 5.0),
        ));
        sketch.entities.insert(Entity::line(
            DVec2::new(20.0, 5.0),
            DVec2::new(20.0, 0.0),
        ));

        let selected: HashSet<EntityId> = [a0, a1, a2, a3].into_iter().collect();
        let near_target = DVec2::new(24.5, 2.6);
        let hit = region_center_snap(&sketch, &selected, near_target, 2.0);
        assert_eq!(hit, Some(DVec2::new(25.0, 2.5)));
    }

    #[test]
    fn ignores_region_belonging_to_excluded_selection() {
        let mut sketch = Sketch::default();
        let a0 = sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
        ));
        let a1 = sketch.entities.insert(Entity::line(
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 5.0),
        ));
        let a2 = sketch.entities.insert(Entity::line(
            DVec2::new(10.0, 5.0),
            DVec2::new(0.0, 5.0),
        ));
        let a3 = sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 5.0),
            DVec2::new(0.0, 0.0),
        ));
        let selected: HashSet<EntityId> = [a0, a1, a2, a3].into_iter().collect();
        let hit = region_center_snap(&sketch, &selected, DVec2::new(5.0, 2.5), 2.0);
        assert_eq!(hit, None);
    }

    #[test]
    fn no_snap_when_outside_tolerance() {
        let mut sketch = Sketch::default();
        sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
        ));
        sketch.entities.insert(Entity::line(
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 5.0),
        ));
        sketch.entities.insert(Entity::line(
            DVec2::new(10.0, 5.0),
            DVec2::new(0.0, 5.0),
        ));
        sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 5.0),
            DVec2::new(0.0, 0.0),
        ));

        let far_target = DVec2::new(500.0, 500.0);
        let hit = region_center_snap(&sketch, &HashSet::new(), far_target, 2.0);
        assert_eq!(hit, None);
    }
}
