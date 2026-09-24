//! Adapter input untuk mode Sketsa Tinta (Concepts-like).
//!
//! Menangani event Touch (Apple Pencil / jari) dan mouse drag,
//! filter latensi adaptif 1€ via [`StrokeBuilder`], palm rejection,
//! dan komit command undo [`AddStroke`].

use ducad_ink::brush::{Brush, BrushId};
use ducad_ink::predict::StrokeBuilder;
use ducad_ink::stroke::{InkPoint, Stroke};
use ducad_sketch::layer::{Layer, LayerId, LayerKind};
use ducad_sketch::style::Rgba;
use ducad_ui::TouchDesignMode;
use eframe::egui;
use glam::Vec2;

use crate::app::DuCADApp;
use crate::ink::tools::InkTool;
use crate::viewport::screen_to_plane_point;

impl DuCADApp {
    /// Mengambil atau menginisialisasi kuas aktif dari preset jika belum ada.
    pub fn get_or_init_active_brush(&mut self) -> BrushId {
        if let Some(id) = self.ink_state.active_brush {
            if self.ink.brushes.contains_key(id) {
                return id;
            }
        }
        if self.ink.brushes.is_empty() {
            for b in Brush::presets() {
                self.ink.brushes.insert(b);
            }
        }
        let id = self.ink.brushes.keys().next().unwrap();
        self.ink_state.active_brush = Some(id);
        id
    }

    /// Mengambil atau membuat layer khusus tinta (`LayerKind::Ink`) pada sketch aktif.
    pub fn get_or_create_ink_layer(&mut self) -> LayerId {
        if let Some(id) = self.ink_state.active_layer {
            return id;
        }
        let found = self.sketch().layers.iter().find_map(|(id, l)| {
            if l.kind == LayerKind::Ink {
                Some(id)
            } else {
                None
            }
        });
        if let Some(id) = found {
            self.ink_state.active_layer = Some(id);
            return id;
        }
        let id = self.sketch_mut().layers.insert(Layer {
            name: "Ink Layer".to_string(),
            visible: true,
            locked: false,
            color: Rgba::BLACK,
            kind: LayerKind::Ink,
        });
        self.ink_state.active_layer = Some(id);
        id
    }

    /// Memproses event sentuhan/stylus individual pada koordinat bidang sketch.
    pub fn handle_ink_touch(
        &mut self,
        phase: egui::TouchPhase,
        plane_pt: Vec2,
        force: Option<f32>,
        touch_id: egui::TouchId,
    ) {
        // Palm rejection: mode PencilOnly mengabaikan sentuhan jari (force = None / 0) untuk menggambar.
        if self.touch_config.mode == TouchDesignMode::PencilOnly {
            let is_pencil = force.map(|f| f > 0.0).unwrap_or(false);
            if !is_pencil {
                return;
            }
        }

        match phase {
            egui::TouchPhase::Start => {
                if self.ink_state.active_touch_id.is_some() {
                    return;
                }
                self.ink_state.active_touch_id = Some(touch_id);
                self.ink_state.stroke_start_instant = Some(std::time::Instant::now());

                match self.ink_state.active_tool {
                    InkTool::Brush => {
                        let brush_id = self.get_or_init_active_brush();
                        let brush = self
                            .ink
                            .brushes
                            .get(brush_id)
                            .cloned()
                            .unwrap_or_else(|| Brush::presets().swap_remove(0));
                        let mut builder = StrokeBuilder::new(&brush);
                        let pressure = force.unwrap_or(0.5).clamp(0.0, 1.0);
                        let raw = InkPoint::new(plane_pt.x, plane_pt.y, pressure, 0.0, 0);
                        if let Some(accepted) = builder.push(raw) {
                            self.ink_state.active_points = vec![accepted];
                        } else {
                            self.ink_state.active_points = vec![raw];
                        }
                        self.ink_state.builder = Some(builder);
                    }
                    InkTool::Eraser | InkTool::Lasso => {
                        self.ink_state.lasso_polygon = vec![plane_pt];
                    }
                    InkTool::Slice => {
                        self.ink_state.slice_line = Some((plane_pt, plane_pt));
                    }
                    InkTool::Nudge | InkTool::Move => {
                        self.ink_state.nudge_center = Some(plane_pt);
                    }
                    InkTool::Color => {}
                }
            }
            egui::TouchPhase::Move => {
                if self.ink_state.active_touch_id != Some(touch_id) {
                    return;
                }
                match self.ink_state.active_tool {
                    InkTool::Brush => {
                        if let Some(builder) = &mut self.ink_state.builder {
                            let t_ms = self
                                .ink_state
                                .stroke_start_instant
                                .map(|i| i.elapsed().as_millis() as u32)
                                .unwrap_or(0);
                            let pressure = force.unwrap_or(0.5).clamp(0.0, 1.0);
                            let raw = InkPoint::new(plane_pt.x, plane_pt.y, pressure, 0.0, t_ms);
                            if let Some(accepted) = builder.push(raw) {
                                self.ink_state.active_points.push(accepted);
                            }
                            self.ink_state.predicted_point = builder.predict(t_ms);
                        }
                    }
                    InkTool::Eraser | InkTool::Lasso => {
                        self.ink_state.lasso_polygon.push(plane_pt);
                    }
                    InkTool::Slice => {
                        if let Some((start, _)) = self.ink_state.slice_line {
                            self.ink_state.slice_line = Some((start, plane_pt));
                        }
                    }
                    InkTool::Nudge => {
                        if let Some(center) = self.ink_state.nudge_center {
                            let delta = plane_pt - center;
                            self.nudge_ink(center, self.ink_state.nudge_radius_mm, delta);
                            self.ink_state.nudge_center = Some(plane_pt);
                        }
                    }
                    InkTool::Move => {
                        if let Some(center) = self.ink_state.nudge_center {
                            let delta = plane_pt - center;
                            self.move_selected_ink(delta);
                            self.ink_state.nudge_center = Some(plane_pt);
                        }
                    }
                    InkTool::Color => {}
                }
            }
            egui::TouchPhase::End => {
                if self.ink_state.active_touch_id != Some(touch_id) {
                    return;
                }
                self.ink_state.active_touch_id = None;
                self.ink_state.predicted_point = None;

                match self.ink_state.active_tool {
                    InkTool::Brush => {
                        if let Some(builder) = self.ink_state.builder.take() {
                            let mut points = builder.finish();
                            if points.is_empty() {
                                points.push(InkPoint::new(
                                    plane_pt.x,
                                    plane_pt.y,
                                    force.unwrap_or(0.5).clamp(0.0, 1.0),
                                    0.0,
                                    0,
                                ));
                            }
                            self.commit_ink_stroke(points);
                        }
                    }
                    InkTool::Eraser => {
                        let path = std::mem::take(&mut self.ink_state.lasso_polygon);
                        self.commit_ink_erase(&path);
                    }
                    InkTool::Lasso => {
                        let poly = std::mem::take(&mut self.ink_state.lasso_polygon);
                        self.commit_ink_lasso(&poly);
                    }
                    InkTool::Slice => {
                        if let Some(line) = self.ink_state.slice_line.take() {
                            self.commit_ink_slice(line);
                        }
                    }
                    InkTool::Nudge | InkTool::Move => {
                        self.ink_state.nudge_center = None;
                    }
                    InkTool::Color => {}
                }
            }
            egui::TouchPhase::Cancel => {
                if self.ink_state.active_touch_id == Some(touch_id) {
                    self.ink_state.active_touch_id = None;
                    self.ink_state.builder = None;
                    self.ink_state.active_points.clear();
                    self.ink_state.predicted_point = None;
                    self.ink_state.lasso_polygon.clear();
                    self.ink_state.slice_line = None;
                    self.ink_state.nudge_center = None;
                }
            }
        }
    }

    /// Komit satu coretan tinta selesai ke dalam dokumen dengan riwayat undo.
    pub fn commit_ink_stroke(&mut self, points: Vec<InkPoint>) {
        if points.is_empty() {
            return;
        }
        let stroke_id = self.ink.next_id;
        let brush_id = self.get_or_init_active_brush();
        let layer_id = self.get_or_create_ink_layer();
        let stroke = Stroke::new(
            stroke_id,
            points,
            brush_id,
            self.ink_state.active_color,
            layer_id,
        );
        self.execute_ink_command(
            Box::new(ducad_ink::commands::AddStroke::new(stroke)),
            "Add Stroke",
        );
        self.ink_state.active_points.clear();
    }

    /// Komit aksi penghapus tinta berdasarkan lintasan `path`.
    pub fn commit_ink_erase(&mut self, path: &[Vec2]) {
        if path.is_empty() {
            return;
        }
        let res = ducad_ink::eraser::erase(&self.ink, path, self.ink_state.erase_mode);
        if !res.removed.is_empty() {
            self.execute_ink_command(
                Box::new(ducad_ink::commands::DeleteStrokes::new(res.removed)),
                "Erase Whole Strokes",
            );
        }
        for (orig_id, pieces) in res.replaced {
            if let Some(stroke) = self.ink.stroke(orig_id).cloned() {
                self.execute_ink_command(
                    Box::new(ducad_ink::commands::DeleteStrokes::new(vec![orig_id])),
                    "Erase Partial Stroke",
                );
                for piece in pieces {
                    let new_id = self.ink.next_id;
                    let new_stroke = Stroke::new(
                        new_id,
                        piece,
                        stroke.brush,
                        stroke.color,
                        stroke.layer,
                    );
                    self.execute_ink_command(
                        Box::new(ducad_ink::commands::AddStroke::new(new_stroke)),
                        "Add Split Stroke Piece",
                    );
                }
            }
        }
    }

    /// Komit seleksi lasso.
    pub fn commit_ink_lasso(&mut self, polygon: &[Vec2]) {
        if polygon.len() < 3 {
            self.ink_state.selected_stroke_ids.clear();
            return;
        }
        self.ink_state.selected_stroke_ids =
            ducad_ink::lasso::lasso_select(&self.ink, polygon, self.ink_state.lasso_mode);
    }

    /// Komit pemotongan garis slice.
    pub fn commit_ink_slice(&mut self, line: (Vec2, Vec2)) {
        let mut to_delete = Vec::new();
        let mut to_add = Vec::new();
        for stroke in &self.ink.strokes {
            let pieces = ducad_ink::slice::slice(&stroke.points, line);
            if pieces.len() > 1 {
                to_delete.push(stroke.id);
                for piece in pieces {
                    to_add.push((piece, stroke.brush, stroke.color, stroke.layer));
                }
            }
        }
        if !to_delete.is_empty() {
            self.execute_ink_command(
                Box::new(ducad_ink::commands::DeleteStrokes::new(to_delete)),
                "Slice Strokes",
            );
            for (points, brush, color, layer) in to_add {
                let new_id = self.ink.next_id;
                let new_stroke = Stroke::new(new_id, points, brush, color, layer);
                self.execute_ink_command(
                    Box::new(ducad_ink::commands::AddStroke::new(new_stroke)),
                    "Add Sliced Piece",
                );
            }
        }
    }

    /// Deformasi lokal titik stroke dalam radius dengan delta pergeseran.
    pub fn nudge_ink(&mut self, center: Vec2, radius: f32, delta: Vec2) {
        if delta.length_squared() < 1e-6 {
            return;
        }
        let hit_ids: Vec<u64> = if !self.ink_state.selected_stroke_ids.is_empty() {
            self.ink_state.selected_stroke_ids.clone()
        } else {
            self.ink.strokes.iter().map(|s| s.id).collect()
        };
        for id in hit_ids {
            if let Some(stroke) = self.ink.stroke(id) {
                let new_points = ducad_ink::nudge::nudge(&stroke.points, center, radius, delta);
                if new_points != stroke.points {
                    self.execute_ink_command(
                        Box::new(ducad_ink::commands::ReplacePoints::new(id, new_points)),
                        "Nudge Stroke Points",
                    );
                }
            }
        }
    }

    /// Menggeser semua stroke terpilih.
    pub fn move_selected_ink(&mut self, delta: Vec2) {
        if self.ink_state.selected_stroke_ids.is_empty() || delta.length_squared() < 1e-6 {
            return;
        }
        let affine = kurbo::Affine::translate((delta.x as f64, delta.y as f64));
        self.execute_ink_command(
            Box::new(ducad_ink::commands::TransformStrokes::new(
                self.ink_state.selected_stroke_ids.clone(),
                affine,
            )),
            "Move Selected Strokes",
        );
    }

    /// Penanganan input kanvas lengkap untuk mode Sketsa Tinta dalam loop frame GUI.
    pub fn handle_ink_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
    ) {
        // 1. Tangani event sentuhan / Apple Pencil nyata
        let touches: Vec<(egui::TouchPhase, Vec2, Option<f32>, egui::TouchId)> = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|e| {
                    if let egui::Event::Touch {
                        id,
                        phase,
                        pos,
                        force,
                        ..
                    } = e
                    {
                        screen_to_plane_point(&self.camera, rect, *pos, &self.active_plane).map(
                            |p| {
                                (
                                    *phase,
                                    Vec2::new(p.x as f32, p.y as f32),
                                    *force,
                                    *id,
                                )
                            },
                        )
                    } else {
                        None
                    }
                })
                .collect()
        });

        if !touches.is_empty() {
            for (phase, pt, force, id) in touches {
                self.handle_ink_touch(phase, pt, force, id);
            }
            ui.ctx().request_repaint();
            return;
        }

        // 2. Fallback pointer mouse/trackpad untuk desktop
        let mouse_id = egui::TouchId(0);
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(p) =
                    screen_to_plane_point(&self.camera, rect, pos, &self.active_plane)
                {
                    let plane_pt = Vec2::new(p.x as f32, p.y as f32);
                    self.handle_ink_touch(egui::TouchPhase::Start, plane_pt, Some(0.5), mouse_id);
                }
            }
        } else if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(p) =
                    screen_to_plane_point(&self.camera, rect, pos, &self.active_plane)
                {
                    let plane_pt = Vec2::new(p.x as f32, p.y as f32);
                    self.handle_ink_touch(egui::TouchPhase::Move, plane_pt, Some(0.5), mouse_id);
                }
            }
        } else if response.drag_stopped() {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(p) =
                    screen_to_plane_point(&self.camera, rect, pos, &self.active_plane)
                {
                    let plane_pt = Vec2::new(p.x as f32, p.y as f32);
                    self.handle_ink_touch(egui::TouchPhase::End, plane_pt, Some(0.5), mouse_id);
                }
            }
        } else if response.clicked() {
            // Ketukan tanpa pergeseran membuat coretan titik (dot stroke)
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(p) =
                    screen_to_plane_point(&self.camera, rect, pos, &self.active_plane)
                {
                    let plane_pt = Vec2::new(p.x as f32, p.y as f32);
                    self.handle_ink_touch(egui::TouchPhase::Start, plane_pt, Some(0.5), mouse_id);
                    self.handle_ink_touch(egui::TouchPhase::End, plane_pt, Some(0.5), mouse_id);
                }
            }
        }

        if self.ink_state.builder.is_some() || !self.ink_state.active_points.is_empty() {
            ui.ctx().request_repaint();
        }
    }
}
