//! Shell desktop CADRAW: jendela eframe + viewport 3D + sketching 2D di
//! bidang XY (Fase 1 + Fase 1 lanjutan).
//!
//! Navigasi kamera:
//! - Drag kiri (tool Pilih) / drag tengah : orbit
//! - Shift+drag / drag kanan              : pan
//! - Scroll / pinch                       : zoom
//! - Dua jari (touch/trackpad)            : orbit + pinch zoom
//!
//! Tool sketch & shortcut: Pilih, Garis (L), Persegi (R), Lingkaran (C),
//! Ellips (E), Arc (A, 3 titik), Offset (O), Mirror (M, perlu seleksi
//! lebih dulu di tool Pilih), Trim (T). Klik menempatkan titik dengan snap
//! otomatis ke endpoint/midpoint/center/intersection/grid; Line/Rectangle/
//! Circle juga menerima dynamic input (ketik panjang/radius + Enter). Esc
//! membatalkan titik pending / kembali ke Pilih. Delete/Backspace menghapus
//! seleksi. Ctrl/Cmd+Z undo, Ctrl/Cmd+Shift+Z atau Ctrl+Y redo.
//!
//! Lingkup yang sengaja belum digarap (bukan lupa — lihat docs/PLAN.md):
//! spline, fillet 2D, extend, offset untuk Ellipse, toleransi snap adaptif
//! mouse-vs-sentuh presisi, interaksi drag-satu-gesture.

use std::collections::HashSet;

use cadraw_render::{sketch as sketch_render, LineVertex, OrbitCamera, SceneRenderer};
use cadraw_sketch::{
    arc_from_three_points, find_snap, line_intersection_params_in_sketch, mirror_entity,
    offset_entity, project_t, trim_segments, DeleteEntities, Entity, EntityId, InsertEntities,
    ReplaceEntities, Sketch, SnapHit,
};
use eframe::egui;
use glam::{DVec2, Mat4, Vec3};

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 32,
        viewport: egui::ViewportBuilder::default()
            .with_title("CADRAW")
            .with_inner_size([1440.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CADRAW",
        options,
        Box::new(|cc| Ok(Box::new(CadrawApp::new(cc)))),
    )
}

/// Tool sketch aktif. Titik yang sudah diklik untuk tool multi-titik
/// disimpan terpisah di `CadrawApp::pending_points` supaya beralih tool
/// tidak perlu memindah state antar varian enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolKind {
    Select,
    Line,
    Rectangle,
    Circle,
    Ellipse,
    /// 3 titik: awal, akhir, titik di busur (menentukan sisi/arah).
    Arc,
    /// Klik entitas sumber, lalu klik sisi & jarak hasil offset.
    Offset,
    /// Perlu seleksi non-kosong (dari tool Pilih) sebelum bisa memilih 2
    /// titik sumbu cermin.
    Mirror,
    /// Klik segmen Line yang mau dipotong di antara/di luar perpotongan
    /// dengan entitas Line lain.
    Trim,
}

/// Berapa titik yang dibutuhkan tool sebelum di-commit lewat
/// `CadrawApp::finish_multipoint`. Offset/Trim ditangani jalur terpisah
/// (bergantung entitas yang diklik, bukan sekadar titik).
fn required_points(tool: ToolKind) -> usize {
    match tool {
        ToolKind::Line | ToolKind::Rectangle | ToolKind::Circle | ToolKind::Ellipse
        | ToolKind::Mirror => 2,
        ToolKind::Arc => 3,
        ToolKind::Select | ToolKind::Offset | ToolKind::Trim => 0,
    }
}

struct CadrawApp {
    camera: OrbitCamera,

    sketch: Sketch,
    undo: cadraw_sketch::UndoStack,

    tool: ToolKind,
    pending_points: Vec<DVec2>,
    /// Entitas sumber untuk tool Offset, di-set pada klik pertama.
    offset_source: Option<EntityId>,

    hovered: Option<EntityId>,
    selected: HashSet<EntityId>,
    last_snap: Option<SnapHit>,

    dynamic_input: String,
    dynamic_focus_pending: bool,
}

impl CadrawApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("CADRAW membutuhkan backend wgpu");
        let scene = SceneRenderer::new(
            &render_state.device,
            render_state.target_format,
            Some(cadraw_render::wgpu::TextureFormat::Depth32Float),
        );
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(scene);
        Self {
            camera: OrbitCamera::default(),
            sketch: Sketch::default(),
            undo: cadraw_sketch::UndoStack::default(),
            tool: ToolKind::Select,
            pending_points: Vec::new(),
            offset_source: None,
            hovered: None,
            selected: HashSet::new(),
            last_snap: None,
            dynamic_input: String::new(),
            dynamic_focus_pending: false,
        }
    }

    fn set_tool(&mut self, tool: ToolKind) {
        self.tool = tool;
        self.pending_points.clear();
        self.offset_source = None;
        self.last_snap = None;
        self.dynamic_input.clear();
        self.dynamic_focus_pending = false;
    }

    fn snapped_or(&self, raw: DVec2) -> DVec2 {
        self.last_snap.map(|s| s.point).unwrap_or(raw)
    }

    /// Terima satu titik klik untuk tool multi-titik aktif; commit otomatis
    /// begitu jumlah titik yang dibutuhkan tool tercapai.
    fn on_click_point(&mut self, p: DVec2) {
        self.pending_points.push(p);
        if self.pending_points.len() == 1 {
            self.dynamic_focus_pending = true;
        }
        if self.pending_points.len() >= required_points(self.tool) {
            self.finish_multipoint();
        }
    }

    /// Bangun entitas/command dari `pending_points` yang sudah lengkap dan
    /// eksekusi lewat undo stack.
    fn finish_multipoint(&mut self) {
        let pts = std::mem::take(&mut self.pending_points);
        let cmd: Option<Box<dyn cadraw_core::Command<Sketch>>> = match self.tool {
            ToolKind::Line => Some(Box::new(InsertEntities::new(
                "Garis",
                vec![Entity::Line {
                    start: pts[0],
                    end: pts[1],
                }],
            ))),
            ToolKind::Rectangle => {
                let min = pts[0].min(pts[1]);
                let max = pts[0].max(pts[1]);
                let corners = [
                    DVec2::new(min.x, min.y),
                    DVec2::new(max.x, min.y),
                    DVec2::new(max.x, max.y),
                    DVec2::new(min.x, max.y),
                ];
                let lines = (0..4)
                    .map(|i| Entity::Line {
                        start: corners[i],
                        end: corners[(i + 1) % 4],
                    })
                    .collect();
                Some(Box::new(InsertEntities::new("Persegi", lines)))
            }
            ToolKind::Circle => {
                let radius = (pts[1] - pts[0]).length();
                (radius > 1e-6).then(|| {
                    Box::new(InsertEntities::new(
                        "Lingkaran",
                        vec![Entity::Circle {
                            center: pts[0],
                            radius,
                        }],
                    )) as Box<dyn cadraw_core::Command<Sketch>>
                })
            }
            ToolKind::Ellipse => {
                let radius_x = (pts[1].x - pts[0].x).abs();
                let radius_y = (pts[1].y - pts[0].y).abs();
                (radius_x > 1e-6 && radius_y > 1e-6).then(|| {
                    Box::new(InsertEntities::new(
                        "Ellips",
                        vec![Entity::Ellipse {
                            center: pts[0],
                            radius_x,
                            radius_y,
                        }],
                    )) as Box<dyn cadraw_core::Command<Sketch>>
                })
            }
            ToolKind::Arc => arc_from_three_points(pts[0], pts[1], pts[2])
                .map(|e| Box::new(InsertEntities::new("Arc", vec![e])) as _),
            ToolKind::Mirror => {
                let (axis_a, axis_b) = (pts[0], pts[1]);
                let mirrored: Vec<Entity> = self
                    .selected
                    .iter()
                    .filter_map(|id| self.sketch.entities.get(*id))
                    .filter_map(|e| mirror_entity(e, axis_a, axis_b))
                    .collect();
                (!mirrored.is_empty())
                    .then(|| Box::new(InsertEntities::new("Cerminkan", mirrored)) as _)
            }
            ToolKind::Select | ToolKind::Offset | ToolKind::Trim => None,
        };
        if let Some(cmd) = cmd {
            self.undo.execute(cmd, &mut self.sketch);
        }
        self.dynamic_input.clear();
        self.dynamic_focus_pending = false;
    }

    fn tool_buttons(&mut self, ui: &mut egui::Ui) {
        for (kind, label) in [
            (ToolKind::Select, "Pilih"),
            (ToolKind::Line, "Garis (L)"),
            (ToolKind::Rectangle, "Persegi (R)"),
            (ToolKind::Circle, "Lingkaran (C)"),
            (ToolKind::Ellipse, "Ellips (E)"),
            (ToolKind::Arc, "Arc (A)"),
            (ToolKind::Offset, "Offset (O)"),
            (ToolKind::Mirror, "Mirror (M)"),
            (ToolKind::Trim, "Trim (T)"),
        ] {
            if ui.selectable_label(self.tool == kind, label).clicked() {
                self.set_tool(kind);
            }
        }
    }

    fn status_text(&self) -> String {
        let hint = match self.tool {
            ToolKind::Select => {
                "Pilih: klik entitas, Shift+klik multi-pilih, Delete hapus".to_string()
            }
            ToolKind::Line => match self.pending_points.len() {
                0 => "Garis: klik titik awal (L)".to_string(),
                _ => "Garis: klik titik akhir, atau ketik panjang lalu Enter".to_string(),
            },
            ToolKind::Rectangle => match self.pending_points.len() {
                0 => "Persegi: klik sudut pertama (R)".to_string(),
                _ => "Persegi: klik sudut berlawanan".to_string(),
            },
            ToolKind::Circle => match self.pending_points.len() {
                0 => "Lingkaran: klik titik pusat (C)".to_string(),
                _ => "Lingkaran: klik untuk radius, atau ketik radius lalu Enter".to_string(),
            },
            ToolKind::Ellipse => match self.pending_points.len() {
                0 => "Ellips: klik titik pusat (E)".to_string(),
                _ => "Ellips: klik sudut kotak pembatas".to_string(),
            },
            ToolKind::Arc => match self.pending_points.len() {
                0 => "Arc: klik titik awal (A)".to_string(),
                1 => "Arc: klik titik akhir".to_string(),
                _ => "Arc: klik titik di busur (menentukan sisi)".to_string(),
            },
            ToolKind::Offset => match self.offset_source {
                None => "Offset: klik entitas sumber (O)".to_string(),
                Some(_) => "Offset: klik sisi & jarak hasil offset".to_string(),
            },
            ToolKind::Mirror => {
                if self.selected.is_empty() {
                    "Mirror: pilih entitas di tool Pilih dulu, lalu tekan M".to_string()
                } else {
                    match self.pending_points.len() {
                        0 => format!(
                            "Mirror: klik titik 1 sumbu cermin ({} entitas terpilih)",
                            self.selected.len()
                        ),
                        _ => "Mirror: klik titik 2 sumbu cermin".to_string(),
                    }
                }
            }
            ToolKind::Trim => "Trim: klik segmen garis yang mau dipotong (T)".to_string(),
        };
        match &self.last_snap {
            Some(snap) => format!("{hint}  ·  snap: {:?}", snap.kind),
            None => hint,
        }
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        let raw_cursor = response
            .hover_pos()
            .and_then(|p| screen_to_plane_point(&self.camera, rect, p));

        // Orbit primer hanya untuk tool Pilih — tool lain memakai klik
        // primer untuk menempatkan titik/memilih entitas. Orbit tetap
        // tersedia lewat drag tengah / dua jari di semua tool.
        self.handle_navigation(ui, &response, rect, self.tool == ToolKind::Select);
        self.handle_sketch_input(ui, &response, rect, raw_cursor);

        let aspect = rect.width() / rect.height().max(1.0);
        let overlay = self.build_overlay_lines(raw_cursor);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                view_proj: self.camera.view_proj(aspect),
                eye: self.camera.eye(),
                overlay_lines: overlay,
            },
        ));

        self.dynamic_input_ui(ui, rect);
    }

    fn handle_navigation(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        allow_primary_orbit: bool,
    ) {
        let delta = response.drag_delta();
        let modifiers = ui.input(|i| i.modifiers);

        let orbiting = (allow_primary_orbit
            && response.dragged_by(egui::PointerButton::Primary)
            && !modifiers.shift)
            || (response.dragged_by(egui::PointerButton::Middle) && !modifiers.shift);
        let panning = response.dragged_by(egui::PointerButton::Secondary)
            || (modifiers.shift
                && (response.dragged_by(egui::PointerButton::Primary)
                    || response.dragged_by(egui::PointerButton::Middle)));

        if panning {
            self.camera.pan(delta.x, delta.y, rect.height());
        } else if orbiting {
            self.camera.orbit(delta.x, delta.y);
        }

        if response.hovered() {
            let pinch = ui.input(|i| i.zoom_delta());
            if pinch != 1.0 {
                self.camera.zoom(pinch);
            }
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom((scroll * 0.003).exp());
            }
        }

        // Dua jari selalu navigasi, terlepas dari tool aktif (gaya Shapr3D:
        // satu jari menggambar/memilih, dua jari mengarahkan kamera).
        if let Some(touch) = ui.input(|i| i.multi_touch()) {
            self.camera
                .orbit(touch.translation_delta.x, touch.translation_delta.y);
            self.camera.zoom(touch.zoom_delta);
        }
    }

    fn handle_sketch_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        raw_cursor: Option<DVec2>,
    ) {
        let text_focused = ui.ctx().memory(|m| m.focused().is_some());

        if !text_focused {
            if !self.selected.is_empty()
                && ui.input(|i| {
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                })
            {
                let ids: Vec<_> = self.selected.drain().collect();
                self.undo
                    .execute(Box::new(DeleteEntities::new(ids)), &mut self.sketch);
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                if !self.pending_points.is_empty() || self.offset_source.is_some() {
                    self.pending_points.clear();
                    self.offset_source = None;
                    self.dynamic_input.clear();
                    self.dynamic_focus_pending = false;
                } else {
                    self.set_tool(ToolKind::Select);
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::L)) {
                self.set_tool(ToolKind::Line);
            }
            if ui.input(|i| i.key_pressed(egui::Key::R)) {
                self.set_tool(ToolKind::Rectangle);
            }
            if ui.input(|i| i.key_pressed(egui::Key::C)) {
                self.set_tool(ToolKind::Circle);
            }
            if ui.input(|i| i.key_pressed(egui::Key::E)) {
                self.set_tool(ToolKind::Ellipse);
            }
            if ui.input(|i| i.key_pressed(egui::Key::A)) {
                self.set_tool(ToolKind::Arc);
            }
            if ui.input(|i| i.key_pressed(egui::Key::O)) {
                self.set_tool(ToolKind::Offset);
            }
            if ui.input(|i| i.key_pressed(egui::Key::M)) {
                self.set_tool(ToolKind::Mirror);
            }
            if ui.input(|i| i.key_pressed(egui::Key::T)) {
                self.set_tool(ToolKind::Trim);
            }
        }

        let Some(raw) = raw_cursor else {
            self.hovered = None;
            self.last_snap = None;
            return;
        };
        let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
        let grid_step = 10.0;

        match self.tool {
            ToolKind::Select => {
                self.last_snap = None;
                self.hovered = response
                    .hovered()
                    .then(|| self.sketch.hit_test(raw, tol))
                    .flatten();
                if response.clicked() {
                    let shift = ui.input(|i| i.modifiers.shift);
                    match (self.hovered, shift) {
                        (Some(hit), true) => {
                            if !self.selected.remove(&hit) {
                                self.selected.insert(hit);
                            }
                        }
                        (Some(hit), false) => {
                            self.selected.clear();
                            self.selected.insert(hit);
                        }
                        (None, false) => self.selected.clear(),
                        (None, true) => {}
                    }
                }
            }
            ToolKind::Line | ToolKind::Rectangle | ToolKind::Circle | ToolKind::Ellipse
            | ToolKind::Arc => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(&self.sketch, raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    let effective = self.snapped_or(raw);
                    self.on_click_point(effective);
                }
            }
            ToolKind::Mirror => {
                self.hovered = None;
                self.last_snap = None;
                if !self.selected.is_empty() {
                    self.last_snap = response
                        .hovered()
                        .then(|| find_snap(&self.sketch, raw, tol, grid_step, None))
                        .flatten();
                    if response.clicked() {
                        let effective = self.snapped_or(raw);
                        self.on_click_point(effective);
                    }
                }
            }
            ToolKind::Offset => {
                self.last_snap = None;
                match self.offset_source {
                    None => {
                        self.hovered = response
                            .hovered()
                            .then(|| self.sketch.hit_test(raw, tol))
                            .flatten();
                        if response.clicked() {
                            self.offset_source = self.hovered;
                        }
                    }
                    Some(source_id) => {
                        self.hovered = None;
                        if response.clicked() {
                            if let Some(entity) = self.sketch.entities.get(source_id) {
                                if let Some(new_entity) = offset_entity(entity, raw) {
                                    self.undo.execute(
                                        Box::new(InsertEntities::new("Offset", vec![new_entity])),
                                        &mut self.sketch,
                                    );
                                }
                            }
                            self.offset_source = None;
                        }
                    }
                }
            }
            ToolKind::Trim => {
                self.last_snap = None;
                // Dibatasi ke entitas Line saja (Fase 1 lanjutan): hit_test
                // global bisa saja menemukan entitas non-Line lebih dekat
                // lalu difilter di sini, jadi kadang tidak memilih Line
                // terdekat kalau ada entitas jenis lain yang lebih dekat —
                // batasan kecil yang bisa disempurnakan nanti (hit-test
                // khusus per-jenis) jika terasa mengganggu.
                self.hovered = response
                    .hovered()
                    .then(|| self.sketch.hit_test(raw, tol))
                    .flatten()
                    .filter(|id| matches!(self.sketch.entities.get(*id), Some(Entity::Line { .. })));
                if response.clicked() {
                    if let Some(id) = self.hovered {
                        if let Some(Entity::Line { start, end }) =
                            self.sketch.entities.get(id).cloned()
                        {
                            let click_t = project_t(start, end, raw).clamp(0.0, 1.0);
                            let cuts =
                                line_intersection_params_in_sketch(&self.sketch, (start, end), id);
                            let remaining = trim_segments(start, end, &cuts, click_t);
                            let new_lines = remaining
                                .into_iter()
                                .map(|(s, e)| Entity::Line { start: s, end: e })
                                .collect();
                            self.undo.execute(
                                Box::new(ReplaceEntities::new("Trim", vec![id], new_lines)),
                                &mut self.sketch,
                            );
                            self.hovered = None;
                        }
                    }
                }
            }
        }
    }

    fn build_overlay_lines(&self, raw_cursor: Option<DVec2>) -> Vec<LineVertex> {
        let mut verts = sketch_render::entity_lines(&self.sketch, self.hovered, &self.selected);

        // Offset: sumber tetap ditandai sebagai preview walau hover pindah.
        if self.tool == ToolKind::Offset {
            if let Some(entity) = self.offset_source.and_then(|id| self.sketch.entities.get(id)) {
                verts.extend(sketch_render::preview_lines(entity));
            }
        }

        if let Some(raw) = raw_cursor {
            match self.tool {
                ToolKind::Line if self.pending_points.len() == 1 => {
                    let preview = Entity::Line {
                        start: self.pending_points[0],
                        end: self.snapped_or(raw),
                    };
                    verts.extend(sketch_render::preview_lines(&preview));
                }
                ToolKind::Rectangle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let min = first.min(effective);
                    let max = first.max(effective);
                    let corners = [
                        DVec2::new(min.x, min.y),
                        DVec2::new(max.x, min.y),
                        DVec2::new(max.x, max.y),
                        DVec2::new(min.x, max.y),
                    ];
                    for i in 0..4 {
                        let preview = Entity::Line {
                            start: corners[i],
                            end: corners[(i + 1) % 4],
                        };
                        verts.extend(sketch_render::preview_lines(&preview));
                    }
                }
                ToolKind::Circle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let preview = Entity::Circle {
                        center: first,
                        radius: (effective - first).length(),
                    };
                    verts.extend(sketch_render::preview_lines(&preview));
                }
                ToolKind::Ellipse if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let radius_x = (effective.x - first.x).abs();
                    let radius_y = (effective.y - first.y).abs();
                    if radius_x > 1e-6 && radius_y > 1e-6 {
                        let preview = Entity::Ellipse {
                            center: first,
                            radius_x,
                            radius_y,
                        };
                        verts.extend(sketch_render::preview_lines(&preview));
                    }
                }
                ToolKind::Arc => {
                    let effective = self.snapped_or(raw);
                    match self.pending_points.len() {
                        1 => {
                            let preview = Entity::Line {
                                start: self.pending_points[0],
                                end: effective,
                            };
                            verts.extend(sketch_render::preview_lines(&preview));
                        }
                        2 => {
                            if let Some(preview) = arc_from_three_points(
                                self.pending_points[0],
                                self.pending_points[1],
                                effective,
                            ) {
                                verts.extend(sketch_render::preview_lines(&preview));
                            }
                        }
                        _ => {}
                    }
                }
                ToolKind::Mirror if !self.selected.is_empty() && self.pending_points.len() == 1 => {
                    let axis_a = self.pending_points[0];
                    let axis_b = self.snapped_or(raw);
                    let axis_preview = Entity::Line {
                        start: axis_a,
                        end: axis_b,
                    };
                    verts.extend(sketch_render::preview_lines(&axis_preview));
                    for entity in self
                        .selected
                        .iter()
                        .filter_map(|id| self.sketch.entities.get(*id))
                    {
                        if let Some(mirrored) = mirror_entity(entity, axis_a, axis_b) {
                            verts.extend(sketch_render::preview_lines(&mirrored));
                        }
                    }
                }
                ToolKind::Offset => {
                    if let Some(entity) =
                        self.offset_source.and_then(|id| self.sketch.entities.get(id))
                    {
                        if let Some(preview) = offset_entity(entity, raw) {
                            verts.extend(sketch_render::preview_lines(&preview));
                        }
                    }
                }
                ToolKind::Trim => {
                    if let Some(id) = self.hovered {
                        if let Some((a, b)) = trim_removal_preview(&self.sketch, id, raw) {
                            verts.extend(sketch_render::removal_preview_lines(a, b));
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(hit) = &self.last_snap {
            verts.extend(sketch_render::snap_glyph(hit));
        }

        verts
    }

    /// Kotak input mengambang di dekat kursor untuk mengetik panjang
    /// (Garis) / radius (Lingkaran) / sisi (Persegi) — dynamic input gaya
    /// AutoCAD. Belum tersedia untuk Ellips/Arc/Offset/Mirror/Trim (lihat
    /// keterbatasan Fase 1 lanjutan di docs/PLAN.md).
    fn dynamic_input_ui(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let supports_dynamic_input =
            matches!(self.tool, ToolKind::Line | ToolKind::Rectangle | ToolKind::Circle);
        if !supports_dynamic_input || self.pending_points.len() != 1 {
            return;
        }
        let first = self.pending_points[0];
        let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };

        egui::Area::new(egui::Id::new("cadraw-dynamic-input"))
            .fixed_pos(cursor + egui::vec2(16.0, 16.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let label = match self.tool {
                        ToolKind::Line => "Panjang (mm)",
                        ToolKind::Circle => "Radius (mm)",
                        ToolKind::Rectangle => "Sisi (mm)",
                        _ => "",
                    };
                    ui.horizontal(|ui| {
                        ui.label(label);
                        let resp = ui.text_edit_singleline(&mut self.dynamic_input);
                        if self.dynamic_focus_pending {
                            resp.request_focus();
                            self.dynamic_focus_pending = false;
                        }
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Ok(value) = self.dynamic_input.trim().parse::<f64>() {
                                if let Some(raw) = screen_to_plane_point(&self.camera, rect, cursor)
                                {
                                    let dir = (raw - first).normalize_or_zero();
                                    let dir = if dir == DVec2::ZERO { DVec2::X } else { dir };
                                    self.on_click_point(first + dir * value);
                                }
                            }
                        }
                    });
                });
            });
    }
}

/// Untuk tool Trim: segmen (awal,akhir) yang akan terhapus jika `hover`
/// diklik sekarang pada entitas Line `id`. Dipakai preview hover; commit
/// klik menghitung ulang lewat `trim_segments` (lihat `handle_sketch_input`)
/// karena butuh daftar lengkap titik potong, bukan cuma satu bracket.
fn trim_removal_preview(sketch: &Sketch, id: EntityId, hover: DVec2) -> Option<(DVec2, DVec2)> {
    let Entity::Line { start, end } = sketch.entities.get(id)?.clone() else {
        return None;
    };
    let click_t = project_t(start, end, hover).clamp(0.0, 1.0);
    let mut ts: Vec<f64> = line_intersection_params_in_sketch(sketch, (start, end), id)
        .into_iter()
        .filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6)
        .collect();
    ts.push(0.0);
    ts.push(1.0);
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.windows(2)
        .find(|w| click_t >= w[0] && click_t <= w[1])
        .map(|w| (start + (end - start) * w[0], start + (end - start) * w[1]))
}

/// Konversi posisi kursor layar → titik di bidang sketch (Z=0), lewat
/// unprojection ray kamera dan interseksi ray-bidang.
fn screen_to_plane_point(camera: &OrbitCamera, rect: egui::Rect, pos: egui::Pos2) -> Option<DVec2> {
    let aspect = rect.width() / rect.height().max(1.0);
    let inv = camera.view_proj(aspect).inverse();

    let ndc_x = ((pos.x - rect.min.x) / rect.width()) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((pos.y - rect.min.y) / rect.height()) * 2.0;

    // Konvensi kedalaman wgpu (Mat4::perspective_rh): NDC z ∈ [0, 1].
    let p_near = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
    let p_far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
    let dir = p_far - p_near;
    if dir.z.abs() < 1e-6 {
        return None; // ray sejajar bidang XY — tidak ada perpotongan berguna
    }
    let t = -p_near.z / dir.z;
    let hit = p_near + dir * t;
    Some(DVec2::new(hit.x as f64, hit.y as f64))
}

/// Perkiraan unit-dunia per piksel layar pada kedalaman target kamera —
/// dipakai mengonversi toleransi hit-test/snap dari piksel ke mm. Toleransi
/// adaptif mouse-vs-sentuh yang lebih presisi menyusul di Fase 4.
fn pixel_tolerance_to_world(camera: &OrbitCamera, rect: egui::Rect) -> f64 {
    let world_per_pixel =
        2.0 * camera.distance * (camera.fov_y * 0.5).tan() / rect.height().max(1.0);
    world_per_pixel as f64
}

impl eframe::App for CadrawApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("CADRAW");
                ui.separator();
                self.tool_buttons(ui);
                ui.separator();
                if ui
                    .add_enabled(self.undo.can_undo(), egui::Button::new("↶ Undo"))
                    .clicked()
                {
                    self.undo.undo(&mut self.sketch);
                }
                if ui
                    .add_enabled(self.undo.can_redo(), egui::Button::new("↷ Redo"))
                    .clicked()
                {
                    self.undo.redo(&mut self.sketch);
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.status_text());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "{} entitas · {} terpilih · kamera {:.0} mm",
                        self.sketch.entities.len(),
                        self.selected.len(),
                        self.camera.distance,
                    ));
                });
            });
        });

        let undo_pressed =
            ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z));
        let redo_pressed = ctx.input(|i| {
            i.modifiers.command
                && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z)))
        });
        if undo_pressed {
            self.undo.undo(&mut self.sketch);
        }
        if redo_pressed {
            self.undo.redo(&mut self.sketch);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.viewport(ui);
            });
    }
}

/// Callback egui_wgpu: jembatan per-frame ke SceneRenderer di
/// `callback_resources`.
struct ViewportCallback {
    view_proj: Mat4,
    eye: Vec3,
    overlay_lines: Vec<LineVertex>,
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
            scene.set_overlay_lines(device, &self.overlay_lines);
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
