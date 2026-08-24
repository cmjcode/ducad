use ducad_render::{
    DraftConfig, LineVertex, OrbitCamera, PlaneKind, SceneRenderer, SketchPlane, ZebraConfig,
};
use ducad_sketch::{EntityId, Sketch};
use eframe::egui;
use eframe::egui_wgpu;
use glam::{DVec2, Mat4, Vec3};

/// Callback render wgpu di viewport egui.
pub struct ViewportCallback {
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub sketch_plane: SketchPlane,
    pub grid_extent: f32,
    pub overlay_lines: Vec<LineVertex>,
    pub body_positions: Vec<[f32; 3]>,
    pub body_normals: Vec<[f32; 3]>,
    pub body_colors: Vec<[f32; 4]>,
    pub body_indices: Vec<u32>,
    pub gizmo_positions: Vec<[f32; 3]>,
    pub gizmo_normals: Vec<[f32; 3]>,
    pub gizmo_colors: Vec<[f32; 4]>,
    pub gizmo_indices: Vec<u32>,
    pub clip_plane: Option<(Vec3, f32)>,
    pub zebra_config: ZebraConfig,
    pub draft_config: DraftConfig,
}

impl egui_wgpu::CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut egui_wgpu::wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<egui_wgpu::wgpu::CommandBuffer> {
        if let Some(scene) = resources.get_mut::<SceneRenderer>() {
            scene.set_grid_plane_with_extent(device, &self.sketch_plane, self.grid_extent, 10.0);
            scene.set_overlay_lines(device, &self.overlay_lines);
            scene.set_mesh(
                device,
                &self.body_positions,
                &self.body_normals,
                Some(&self.body_colors),
                &self.body_indices,
            );
            scene.set_gizmo_mesh(
                device,
                &self.gizmo_positions,
                &self.gizmo_normals,
                &self.gizmo_colors,
                &self.gizmo_indices,
            );
            scene.set_clip_plane(self.clip_plane);
            scene.set_zebra_config(self.zebra_config);
            scene.set_draft_config(self.draft_config);
            scene.prepare(queue, self.view_proj, self.eye);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        rpass: &mut egui_wgpu::wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(scene) = resources.get::<SceneRenderer>() {
            scene.paint(rpass);
        }
    }
}

/// Ray dunia (titik dekat + arah) dari posisi kursor layar.
pub fn screen_to_ray(camera: &OrbitCamera, rect: egui::Rect, pos: egui::Pos2) -> (Vec3, Vec3) {
    let aspect = rect.width() / rect.height().max(1.0);
    let inv = camera.view_proj(aspect).inverse();

    let ndc_x = ((pos.x - rect.min.x) / rect.width()) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((pos.y - rect.min.y) / rect.height()) * 2.0;

    let p_near = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
    let p_far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
    (p_near, p_far - p_near)
}

/// Cari entitas sketch yang di-hit di titik 2D `p` dengan selection cycling.
pub fn hit_test_cycled(sketch: &Sketch, p: DVec2, tolerance: f64, cycle: usize) -> Option<EntityId> {
    let mut candidates: Vec<(EntityId, f64)> = sketch
        .entities
        .iter()
        .filter(|(id, _)| !sketch.is_hidden(*id))
        .map(|(id, e)| (id, e.distance_to(p)))
        .filter(|(_, d)| *d <= tolerance)
        .collect();
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    Some(candidates[cycle % candidates.len()].0)
}

/// Konversi posisi kursor layar → titik di bidang sketch aktif.
pub fn screen_to_plane_point(
    camera: &OrbitCamera,
    rect: egui::Rect,
    pos: egui::Pos2,
    plane: &SketchPlane,
) -> Option<DVec2> {
    let (p_near, dir) = screen_to_ray(camera, rect, pos);
    plane.ray_intersection(p_near, dir)
}

/// Cari bidang sketsa NON-AKTIF yang kena ray dari posisi kursor/sentuhan layar.
pub fn pick_inactive_plane_at_cursor(
    camera: &OrbitCamera,
    rect: egui::Rect,
    pos: egui::Pos2,
    active_kind: PlaneKind,
) -> Option<PlaneKind> {
    let (p_near, dir) = screen_to_ray(camera, rect, pos);
    pick_inactive_plane_for_ray(p_near, dir, active_kind)
}

/// Bagian murni-matematika dari `pick_inactive_plane_at_cursor`.
pub fn pick_inactive_plane_for_ray(
    p_near: Vec3,
    dir: Vec3,
    active_kind: PlaneKind,
) -> Option<PlaneKind> {
    let half_extent = ducad_render::grid::INACTIVE_PLANE_HALF_EXTENT as f64;

    let mut best: Option<(PlaneKind, f64)> = None;
    for kind in PlaneKind::all() {
        if kind == active_kind {
            continue;
        }
        let plane = SketchPlane::from_kind(kind);
        let Some(uv) = plane.ray_intersection(p_near, dir) else {
            continue;
        };
        if uv.x.abs() > half_extent || uv.y.abs() > half_extent {
            continue;
        }
        let hit = plane.to_world(uv, 0.0);
        let dist = (hit - p_near).length() as f64;
        if best.is_none_or(|(_, best_dist)| dist < best_dist) {
            best = Some((kind, dist));
        }
    }
    best.map(|(kind, _)| kind)
}

/// Perkiraan unit-dunia per piksel layar pada kedalaman target kamera.
pub fn pixel_tolerance_to_world(camera: &OrbitCamera, rect: egui::Rect) -> f64 {
    let world_per_pixel =
        2.0 * camera.distance * (camera.fov_y * 0.5).tan() / rect.height().max(1.0);
    world_per_pixel as f64
}

/// Proyeksikan titik 3D dunia ke koordinat piksel layar egui.
pub fn world_to_screen_pos(
    camera: &OrbitCamera,
    rect: egui::Rect,
    world_pt: Vec3,
) -> Option<egui::Pos2> {
    let aspect = rect.width() / rect.height().max(1.0);
    let vp = camera.view_proj(aspect);
    let clip = vp.project_point3(world_pt);
    if clip.z < 0.0 || clip.z > 1.0 {
        return None;
    }
    let screen_x = rect.min.x + (clip.x + 1.0) * 0.5 * rect.width();
    let screen_y = rect.min.y + (1.0 - clip.y) * 0.5 * rect.height();
    Some(egui::pos2(screen_x, screen_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_click_ray_hits_front_plane_activates_it() {
        let p_near = Vec3::new(15.0, -100.0, 25.0);
        let dir = Vec3::new(0.0, 1.0, 0.0);
        let hit = pick_inactive_plane_for_ray(p_near, dir, PlaneKind::Top);
        assert_eq!(hit, Some(PlaneKind::Front));
    }

    #[test]
    fn cmd_click_ray_hits_right_plane_activates_it() {
        let p_near = Vec3::new(100.0, 30.0, 40.0);
        let dir = Vec3::new(-1.0, 0.0, 0.0);
        let hit = pick_inactive_plane_for_ray(p_near, dir, PlaneKind::Top);
        assert_eq!(hit, Some(PlaneKind::Right));
    }

    #[test]
    fn active_plane_never_returned_even_if_ray_hits_it() {
        let p_near = Vec3::new(15.0, -100.0, 25.0);
        let dir = Vec3::new(0.0, 1.0, 0.0);
        let hit = pick_inactive_plane_for_ray(p_near, dir, PlaneKind::Front);
        assert_ne!(hit, Some(PlaneKind::Front));
    }

    #[test]
    fn hit_outside_half_extent_is_ignored() {
        let half_extent = ducad_render::grid::INACTIVE_PLANE_HALF_EXTENT;
        let far = half_extent * 3.0;
        let p_near = Vec3::new(far, -100.0, far);
        let dir = Vec3::new(0.0, 1.0, 0.0);
        let hit = pick_inactive_plane_for_ray(p_near, dir, PlaneKind::Top);
        assert_eq!(hit, None);
    }

    #[test]
    fn parallel_ray_to_both_inactive_planes_returns_none() {
        let p_near = Vec3::new(0.0, 0.0, 50.0);
        let dir = Vec3::new(1.0, 0.0, 0.0);
        let hit = pick_inactive_plane_for_ray(p_near, dir, PlaneKind::Right);
        assert_eq!(hit, None);
    }
}
