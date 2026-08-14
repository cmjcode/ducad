//! Shell desktop CADRAW: jendela eframe + viewport 3D + sketching 2D
//! (Fase 1) di bidang XY.
//!
//! Navigasi kamera:
//! - Drag kiri (tool Pilih) / drag tengah : orbit
//! - Shift+drag / drag kanan              : pan
//! - Scroll / pinch                       : zoom
//! - Dua jari (touch/trackpad)            : orbit + pinch zoom
//!
//! Sketching (tool Garis/Persegi/Lingkaran aktif — lihat toolbar/status
//! bar): klik untuk menempatkan titik, snap otomatis ke endpoint/midpoint/
//! center/intersection/grid, atau ketik panjang/radius lalu Enter (dynamic
//! input, gaya AutoCAD). Esc membatalkan titik pending. Delete/Backspace
//! menghapus entitas terpilih. Ctrl/Cmd+Z undo, Ctrl/Cmd+Shift+Z (atau
//! Ctrl+Y) redo.
//!
//! Lingkup Fase 1 (sengaja dibatasi): tool Pilih/Garis/Persegi/Lingkaran,
//! snap endpoint/midpoint/center/intersection/grid, dynamic input
//! panjang/radius. Arc/ellipse/spline/fillet-2D/trim/offset/mirror dan
//! constraint solver menyusul di iterasi Fase 1 berikutnya & Fase 2.

use std::collections::HashSet;

use cadraw_render::{sketch as sketch_render, LineVertex, OrbitCamera, SceneRenderer};
use cadraw_sketch::{
    find_snap, DeleteEntities, Entity, EntityId, InsertEntities, Sketch, SnapHit,
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

/// Tool sketch aktif. Titik pending (untuk tool 2-klik) disimpan terpisah
/// di `CadrawApp::pending_first` supaya beralih tool tidak perlu memindah
/// state antar varian enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolKind {
    Select,
    Line,
    Rectangle,
    Circle,
}

struct CadrawApp {
    camera: OrbitCamera,

    sketch: Sketch,
    undo: cadraw_sketch::UndoStack,

    tool: ToolKind,
    /// Titik pertama yang sudah ditempatkan untuk tool Garis/Persegi/
    /// Lingkaran; `None` berarti belum ada klik pertama.
    pending_first: Option<DVec2>,

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
            pending_first: None,
            hovered: None,
            selected: HashSet::new(),
            last_snap: None,
            dynamic_input: String::new(),
            dynamic_focus_pending: false,
        }
    }

    fn set_tool(&mut self, tool: ToolKind) {
        self.tool = tool;
        self.pending_first = None;
        self.last_snap = None;
        self.dynamic_input.clear();
        self.dynamic_focus_pending = false;
    }

    /// Selesaikan tool 2-titik: sisipkan entitas via command undo-able.
    fn commit_tool(&mut self, first: DVec2, second: DVec2) {
        let cmd: Option<Box<dyn cadraw_core::Command<Sketch>>> = match self.tool {
            ToolKind::Line => Some(Box::new(InsertEntities::new(
                "Garis",
                vec![Entity::Line {
                    start: first,
                    end: second,
                }],
            ))),
            ToolKind::Rectangle => {
                let min = first.min(second);
                let max = first.max(second);
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
                let radius = (second - first).length();
                (radius > 1e-6).then(|| {
                    Box::new(InsertEntities::new(
                        "Lingkaran",
                        vec![Entity::Circle {
                            center: first,
                            radius,
                        }],
                    )) as Box<dyn cadraw_core::Command<Sketch>>
                })
            }
            ToolKind::Select => None,
        };
        if let Some(cmd) = cmd {
            self.undo.execute(cmd, &mut self.sketch);
        }
        self.pending_first = None;
        self.dynamic_input.clear();
        self.dynamic_focus_pending = false;
    }

    fn tool_buttons(&mut self, ui: &mut egui::Ui) {
        for (kind, label) in [
            (ToolKind::Select, "Pilih"),
            (ToolKind::Line, "Garis (L)"),
            (ToolKind::Rectangle, "Persegi (R)"),
            (ToolKind::Circle, "Lingkaran (C)"),
        ] {
            if ui.selectable_label(self.tool == kind, label).clicked() {
                self.set_tool(kind);
            }
        }
    }

    fn status_text(&self) -> String {
        let hint = match (self.tool, self.pending_first) {
            (ToolKind::Select, _) => {
                "Pilih: klik entitas, Shift+klik multi-pilih, Delete hapus".to_string()
            }
            (ToolKind::Line, None) => "Garis: klik titik awal (L)".to_string(),
            (ToolKind::Line, Some(_)) => {
                "Garis: klik titik akhir, atau ketik panjang lalu Enter".to_string()
            }
            (ToolKind::Rectangle, None) => "Persegi: klik sudut pertama (R)".to_string(),
            (ToolKind::Rectangle, Some(_)) => "Persegi: klik sudut berlawanan".to_string(),
            (ToolKind::Circle, None) => "Lingkaran: klik titik pusat (C)".to_string(),
            (ToolKind::Circle, Some(_)) => {
                "Lingkaran: klik untuk radius, atau ketik radius lalu Enter".to_string()
            }
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

        // Orbit primer hanya untuk tool Pilih — saat tool gambar aktif,
        // klik primer dipakai menempatkan titik, bukan navigasi. Orbit
        // tetap tersedia lewat drag tengah / dua jari.
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
        // satu jari menggambar, dua jari mengarahkan kamera).
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
                && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
            {
                let ids: Vec<_> = self.selected.drain().collect();
                self.undo
                    .execute(Box::new(DeleteEntities::new(ids)), &mut self.sketch);
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                if self.pending_first.is_some() {
                    self.pending_first = None;
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
            ToolKind::Line | ToolKind::Rectangle | ToolKind::Circle => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(&self.sketch, raw, tol, grid_step, None))
                    .flatten();
                let effective = self.last_snap.map(|s| s.point).unwrap_or(raw);

                if response.clicked() {
                    match self.pending_first {
                        None => {
                            self.pending_first = Some(effective);
                            self.dynamic_focus_pending = true;
                        }
                        Some(first) => self.commit_tool(first, effective),
                    }
                }
            }
        }
    }

    fn build_overlay_lines(&self, raw_cursor: Option<DVec2>) -> Vec<LineVertex> {
        let mut verts = sketch_render::entity_lines(&self.sketch, self.hovered, &self.selected);

        if let (Some(first), Some(raw)) = (self.pending_first, raw_cursor) {
            let effective = self.last_snap.map(|s| s.point).unwrap_or(raw);
            match self.tool {
                ToolKind::Line => {
                    let preview = Entity::Line {
                        start: first,
                        end: effective,
                    };
                    verts.extend(sketch_render::preview_lines(&preview));
                }
                ToolKind::Circle => {
                    let preview = Entity::Circle {
                        center: first,
                        radius: (effective - first).length(),
                    };
                    verts.extend(sketch_render::preview_lines(&preview));
                }
                ToolKind::Rectangle => {
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
                ToolKind::Select => {}
            }
        }

        if let Some(hit) = &self.last_snap {
            verts.extend(sketch_render::snap_glyph(hit));
        }

        verts
    }

    /// Kotak input mengambang di dekat kursor untuk mengetik panjang
    /// (Garis) atau radius (Lingkaran) — dynamic input gaya AutoCAD.
    fn dynamic_input_ui(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let Some(first) = self.pending_first else {
            return;
        };
        if self.tool == ToolKind::Select {
            return;
        }
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
                        ToolKind::Select => "",
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
                                    self.commit_tool(first, first + dir * value);
                                }
                            }
                        }
                    });
                });
            });
    }
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
            ui.horizontal(|ui| {
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
