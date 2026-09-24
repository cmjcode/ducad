//! Adapter input Mode Vektor: tool Pen Bézier (M2.2) dan Node Edit (M2.3).
//!
//! Logika dipisah dari egui: event pointer dinyatakan sebagai
//! [`VectorPointer`] dalam koordinat bidang (mm), sehingga urutan event bisa
//! disimulasikan di tes tanpa GUI. `handle_vector_tool_input` hanya
//! menerjemahkan `egui::Response` menjadi event tersebut.

use ducad_render::LineVertex;
use ducad_sketch::commands::{node_coalesce_key, DeleteEntities, UpdateEntity};
use ducad_sketch::path_edit::HandleSide;
use ducad_sketch::{find_snap, Entity, EntityId};
use eframe::egui;
use glam::DVec2;

use crate::app::DuCADApp;
use crate::types::ToolKind;
use crate::vector::{
    AlignMode, AlignRelative, AlignTool, DistributeMode, DragState, DragTarget, NodeEditTool,
    PenTool, PivotAnchor, PrecisionTransform, TransformDialog, TransformParams,
};
use crate::viewport::screen_to_plane_point;

/// Event pointer tool vektor dalam koordinat bidang aktif (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VectorPointer {
    Down(DVec2),
    Drag(DVec2),
    Up(DVec2),
}

/// Modifier yang relevan untuk tool vektor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VectorMods {
    /// Pen: kunci sudut 45°. Node Edit: tambah/kurangi seleksi.
    pub shift: bool,
    /// Pen: patahkan handle. Node Edit: geser handle tanpa menjaga kehalusan.
    pub alt: bool,
}

/// Label command undo untuk edit node (satu drag = satu langkah lewat coalescing).
const NODE_EDIT_LABEL: &str = "Edit Node";
/// Jarak grid snap Pen (mm), sama dengan tool sketch lain.
const GRID_STEP: f64 = 10.0;

impl DuCADApp {
    // ------------------------------------------------------------------
    // Pen Bézier
    // ------------------------------------------------------------------

    /// Proses satu event pointer tool Pen. Mengembalikan id path bila event
    /// ini menutup subpath (klik node awal) dan path di-commit.
    pub fn pen_pointer(
        &mut self,
        ev: VectorPointer,
        mods: VectorMods,
        tol: f64,
    ) -> Option<EntityId> {
        match ev {
            VectorPointer::Down(p) => {
                let snap = find_snap(self.sketch(), p, tol, GRID_STEP, None).map(|s| s.point);
                let pen = &mut self.vector_state.pen;
                let before = pen.builder.clone();
                let closed =
                    PenTool::handle_pointer_down(&mut pen.builder, p, mods.shift, snap, tol);
                if let Some(sub) = closed {
                    self.vector_state.pen = Default::default();
                    return self.commit_pen_subpath(sub);
                }
                pen.before_node = Some(before);
                pen.drag_start = pen.builder.last_point();
                pen.is_dragging = true;
                None
            }
            VectorPointer::Drag(p) => {
                let pen = &mut self.vector_state.pen;
                if let (true, Some(before), Some(node)) =
                    (pen.is_dragging, pen.before_node.clone(), pen.drag_start)
                {
                    // Getaran kecil saat klik bukan niat membuat handle.
                    if (p - node).length() > tol {
                        PenTool::handle_pointer_drag(&mut pen.builder, &before, node, p, mods.alt);
                    }
                }
                None
            }
            VectorPointer::Up(_) => {
                let pen = &mut self.vector_state.pen;
                pen.is_dragging = false;
                pen.before_node = None;
                None
            }
        }
    }

    /// Enter/Esc: akhiri path terbuka dan commit bila minimal dua node.
    pub fn pen_finish(&mut self) -> Option<EntityId> {
        let pen = std::mem::take(&mut self.vector_state.pen);
        pen.builder
            .finish(false)
            .and_then(|sub| self.commit_pen_subpath(sub))
    }

    /// Backspace: batalkan node terakhir yang belum di-commit.
    pub fn pen_undo_node(&mut self) -> bool {
        self.vector_state.pen.builder.pop()
    }

    // ------------------------------------------------------------------
    // Node Edit
    // ------------------------------------------------------------------

    /// Path terlihat di sketsa aktif, dari yang paling atas (draw order terbalik).
    fn visible_paths_topmost_first(&self) -> Vec<EntityId> {
        let sketch = self.sketch();
        sketch
            .draw_order()
            .into_iter()
            .rev()
            .filter(|id| !sketch.is_hidden(*id))
            .filter(|id| matches!(sketch.entities.get(*id), Some(Entity::Path { .. })))
            .collect()
    }

    fn hit_node_edit_target(&self, p: DVec2, tol: f64) -> Option<DragTarget> {
        let sketch = self.sketch();
        let paths = self.visible_paths_topmost_first();
        // Handle hanya untuk path yang node-nya sedang dipilih (seperti CorelDraw).
        let selected_entities: Vec<EntityId> = self
            .vector_state
            .node_selection
            .iter()
            .map(|(e, _, _)| *e)
            .collect();
        for &id in paths.iter().filter(|id| selected_entities.contains(id)) {
            let Some(Entity::Path { subpaths, .. }) = sketch.entities.get(id) else {
                continue;
            };
            for (s_idx, sub) in subpaths.iter().enumerate() {
                if let Some((node, side)) = NodeEditTool::hit_test_handle(sub, p, tol) {
                    return Some(DragTarget::Handle {
                        entity: id,
                        subpath: s_idx as u16,
                        node: node as u32,
                        side,
                    });
                }
            }
        }
        for id in paths {
            let Some(Entity::Path { subpaths, .. }) = sketch.entities.get(id) else {
                continue;
            };
            for (s_idx, sub) in subpaths.iter().enumerate() {
                if let Some(node) = NodeEditTool::hit_test_node(sub, p, tol) {
                    return Some(DragTarget::Node {
                        entity: id,
                        subpath: s_idx as u16,
                        node: node as u32,
                    });
                }
            }
        }
        None
    }

    /// Proses satu event pointer tool Node Edit. Setiap drag menghasilkan
    /// `UpdateEntity` dengan kunci coalescing `node-drag`, sehingga satu tarikan
    /// = satu langkah undo.
    pub fn node_edit_pointer(&mut self, ev: VectorPointer, mods: VectorMods, tol: f64) {
        match ev {
            VectorPointer::Down(p) => {
                let target = self.hit_node_edit_target(p, tol);
                let vs = &mut self.vector_state;
                match target {
                    Some(DragTarget::Node {
                        entity,
                        subpath,
                        node,
                    }) => {
                        if mods.shift {
                            vs.toggle_node_selection(entity, subpath, node);
                        } else if !vs.node_selection.contains(&(entity, subpath, node)) {
                            vs.select_single_node(entity, subpath, node);
                        }
                    }
                    Some(DragTarget::Handle { .. }) => {}
                    Some(DragTarget::Entity(_)) | None => {
                        if !mods.shift {
                            vs.clear_node_selection();
                        }
                    }
                }
                vs.drag = target.map(|target| DragState {
                    target,
                    start_world: p,
                    current_world: p,
                });
            }
            VectorPointer::Drag(p) => {
                let Some(drag) = self.vector_state.drag.as_mut() else {
                    return;
                };
                drag.current_world = p;
                let target = drag.target;
                self.apply_node_drag(target, p, mods);
            }
            VectorPointer::Up(_) => {
                self.vector_state.drag = None;
            }
        }
    }

    fn apply_node_drag(&mut self, target: DragTarget, p: DVec2, mods: VectorMods) {
        match target {
            DragTarget::Node {
                entity,
                subpath,
                node,
            } => {
                let Some(current) = self.sketch().entities.get(entity).cloned() else {
                    return;
                };
                let Entity::Path { subpaths, .. } = &current else {
                    return;
                };
                let Some(from) = subpaths
                    .get(subpath as usize)
                    .and_then(|s| s.node(node as usize))
                else {
                    return;
                };
                let delta = p - from;
                // Semua node terpilih pada path ini ikut bergeser sebesar delta.
                let mut moved = current.clone();
                for &(e, s, n) in &self.vector_state.node_selection {
                    if e != entity {
                        continue;
                    }
                    let Entity::Path { subpaths, .. } = &moved else {
                        break;
                    };
                    let Some(pos) = subpaths.get(s as usize).and_then(|sp| sp.node(n as usize))
                    else {
                        continue;
                    };
                    if let Some(next) = NodeEditTool::move_node_on_entity(
                        &moved,
                        s as usize,
                        n as usize,
                        pos + delta,
                    ) {
                        moved = next;
                    }
                }
                self.exec_node_update(entity, moved, node_coalesce_key(entity, subpath, node));
            }
            DragTarget::Handle {
                entity,
                subpath,
                node,
                side,
            } => {
                let Some(current) = self.sketch().entities.get(entity) else {
                    return;
                };
                if let Some(next) = NodeEditTool::move_handle_on_entity(
                    current,
                    subpath as usize,
                    node as usize,
                    side,
                    p,
                    !mods.alt,
                ) {
                    let side_bit = u32::from(side == HandleSide::Out) << 31;
                    let key = node_coalesce_key(entity, subpath, node | side_bit);
                    self.exec_node_update(entity, next, key);
                }
            }
            DragTarget::Entity(_) => {}
        }
    }

    fn exec_node_update(&mut self, id: EntityId, entity: Entity, key: (&'static str, u64)) {
        if self.sketch().entities.get(id) == Some(&entity) {
            return;
        }
        let cmd = UpdateEntity::new(NODE_EDIT_LABEL, id, entity).with_coalesce_key(key.0, key.1);
        self.execute_sketch_command(Box::new(cmd));
    }

    /// Delete/Backspace pada Node Edit: hapus node terpilih. Path yang
    /// kehabisan node (tinggal < 2) dihapus seluruhnya.
    pub fn node_edit_delete_selected(&mut self) {
        // Urutan menurun (BTreeSet terbalik) agar indeks node sisanya tetap sah.
        let selection: Vec<(EntityId, u16, u32)> = self
            .vector_state
            .node_selection
            .iter()
            .rev()
            .copied()
            .collect();
        self.vector_state.clear_node_selection();
        let mut entities: Vec<EntityId> = selection.iter().map(|(e, _, _)| *e).collect();
        entities.dedup();
        for id in entities {
            let Some(mut entity) = self.sketch().entities.get(id).cloned() else {
                continue;
            };
            let mut emptied = false;
            for &(_, s, n) in selection.iter().filter(|(e, _, _)| *e == id) {
                match NodeEditTool::delete_node_on_entity(&entity, s as usize, n as usize) {
                    Some(next) => entity = next,
                    None => emptied = true,
                }
            }
            if emptied {
                self.execute_sketch_command(Box::new(DeleteEntities::new(vec![id])));
            } else {
                self.exec_node_update(id, entity, ("node-delete", 0));
            }
        }
    }

    /// Constraint yang tersedia untuk seleksi node saat ini (M2.6).
    pub fn node_constraint_actions(&self) -> Vec<ducad_ui::ConstraintAction> {
        crate::vector::node_edit::valid_constraints_for_node_count(
            self.vector_state.node_selection.len(),
        )
    }

    /// Pasang constraint dari constraint strip pada node terpilih (M2.6).
    /// Lewat `apply_constraint`: dicek solver dulu, lalu satu command undo.
    pub fn apply_node_constraint(&mut self, action: ducad_ui::ConstraintAction) {
        let nodes: Vec<(EntityId, u16, u32)> =
            self.vector_state.node_selection.iter().copied().collect();
        match crate::vector::node_edit::build_node_constraint(self.sketch(), &nodes, action) {
            Some(c) => self.apply_constraint(c),
            None => {
                self.constraint_status =
                    Some("Constraint ini tidak berlaku untuk node yang dipilih".to_string());
            }
        }
    }

    // ------------------------------------------------------------------
    // Align / Distribute / Transform presisi (M2.5)
    // ------------------------------------------------------------------

    /// Seleksi entitas terurut draw order — `selected` adalah `HashSet`, dan
    /// urutan menentukan hasil distribusi & label undo yang deterministik.
    fn selection_in_draw_order(&self) -> Vec<EntityId> {
        self.sketch()
            .draw_order()
            .into_iter()
            .filter(|id| self.selected.contains(id))
            .collect()
    }

    /// Ratakan seleksi terhadap kotak batas seleksi (satu langkah undo).
    pub fn align_selection(&mut self, mode: AlignMode) {
        let ids = self.selection_in_draw_order();
        if ids.len() < 2 {
            return;
        }
        if let Some(cmd) = AlignTool::align(self.sketch(), &ids, mode, AlignRelative::Selection) {
            self.execute_sketch_command(Box::new(cmd));
        }
    }

    /// Sebar pusat seleksi merata (minimal 3 objek; satu langkah undo).
    pub fn distribute_selection(&mut self, mode: DistributeMode) {
        let ids = self.selection_in_draw_order();
        if let Some(cmd) = AlignTool::distribute(self.sketch(), &ids, mode) {
            self.execute_sketch_command(Box::new(cmd));
        }
    }

    /// Buka dialog transformasi presisi, terisi kotak batas seleksi saat ini.
    pub fn open_transform_dialog(&mut self) {
        let ids = self.selection_in_draw_order();
        let Some((min, max)) =
            AlignTool::compute_target_bounds(self.sketch(), &ids, AlignRelative::Selection)
        else {
            return;
        };
        let mid = (min + max) * 0.5;
        let size = max - min;
        self.vector_state.transform_dialog = Some(TransformDialog {
            x: format!("{:.3}", mid.x),
            y: format!("{:.3}", mid.y),
            w: format!("{:.3}", size.x),
            h: format!("{:.3}", size.y),
            rot_deg: "0".to_string(),
            lock_aspect: false,
        });
    }

    /// Terapkan isian dialog ke seleksi. Mengembalikan pesan error bila
    /// isian tidak valid; seleksi dipindah ke entitas hasil transformasi.
    pub fn apply_transform_dialog(&mut self, dialog: &TransformDialog) -> Result<(), String> {
        let num = |label: &str, v: &str| -> Result<f64, String> {
            v.trim()
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite())
                .ok_or_else(|| format!("{label} bukan angka: '{v}'"))
        };
        let (w, h) = (num("Lebar", &dialog.w)?, num("Tinggi", &dialog.h)?);
        if w <= 0.0 || h <= 0.0 {
            return Err("Lebar dan tinggi harus > 0".to_string());
        }
        let params = TransformParams {
            target_x: Some(num("X", &dialog.x)?),
            target_y: Some(num("Y", &dialog.y)?),
            target_w: Some(w),
            target_h: (!dialog.lock_aspect).then_some(h),
            rotation_rad: Some(num("Rotasi", &dialog.rot_deg)?.to_radians()),
            lock_aspect_ratio: dialog.lock_aspect,
            pivot: PivotAnchor::Center,
            ..TransformParams::default()
        };
        let ids = self.selection_in_draw_order();
        let cmd = PrecisionTransform::build_transform_command(self.sketch(), &ids, &params)
            .ok_or_else(|| "Seleksi tidak punya geometri untuk ditransformasi".to_string())?;
        let before: std::collections::HashSet<EntityId> = self.sketch().entities.keys().collect();
        self.execute_sketch_command(Box::new(cmd));
        // `ReplaceEntities` memberi id baru: seleksi mengikuti hasilnya.
        self.selected = self
            .sketch()
            .entities
            .keys()
            .filter(|id| !before.contains(id))
            .collect();
        Ok(())
    }

    /// Jendela dialog transformasi presisi (dipanggil tiap frame).
    pub fn show_transform_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.vector_state.transform_dialog.take() else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Transformasi Presisi")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::Grid::new("ducad-transform-grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (label, field) in [
                            ("Pusat X (mm)", &mut dialog.x),
                            ("Pusat Y (mm)", &mut dialog.y),
                            ("Lebar (mm)", &mut dialog.w),
                            ("Tinggi (mm)", &mut dialog.h),
                            ("Rotasi (°)", &mut dialog.rot_deg),
                        ] {
                            ui.label(label);
                            ui.text_edit_singleline(field);
                            ui.end_row();
                        }
                    });
                ui.checkbox(
                    &mut dialog.lock_aspect,
                    "Kunci rasio (tinggi mengikuti lebar)",
                );
                apply = ui.button("Terapkan").clicked();
            });
        if apply {
            match self.apply_transform_dialog(&dialog) {
                Ok(()) => return,
                Err(e) => self.constraint_status = Some(e),
            }
        }
        if open {
            self.vector_state.transform_dialog = Some(dialog);
        }
    }

    // ------------------------------------------------------------------
    // Overlay & glue egui
    // ------------------------------------------------------------------

    /// Garis overlay tool vektor (pratinjau Pen; node & handle Node Edit),
    /// sudah dipetakan ke koordinat dunia bidang aktif.
    pub fn vector_overlay_lines(&self, cursor: Option<DVec2>, world_scale: f64) -> Vec<LineVertex> {
        let mut lines = Vec::new();
        match self.tool {
            ToolKind::PenBezier => {
                if let Some(c) = cursor {
                    lines = PenTool::preview_overlay_lines(
                        &self.vector_state.pen.builder,
                        c,
                        false,
                        [1.0, 0.8, 0.1, 1.0],
                    );
                }
            }
            ToolKind::NodeEdit => {
                let sketch = self.sketch();
                let mut ids: Vec<EntityId> = self
                    .vector_state
                    .node_selection
                    .iter()
                    .map(|(e, _, _)| *e)
                    .collect();
                ids.extend(self.selected.iter().copied());
                ids.sort();
                ids.dedup();
                for id in ids {
                    let Some(Entity::Path { subpaths, .. }) = sketch.entities.get(id) else {
                        continue;
                    };
                    for (s_idx, sub) in subpaths.iter().enumerate() {
                        let selected: Vec<usize> = self
                            .vector_state
                            .node_selection
                            .iter()
                            .filter(|(e, s, _)| *e == id && *s as usize == s_idx)
                            .map(|(_, _, n)| *n as usize)
                            .collect();
                        lines.extend(NodeEditTool::overlay_lines(
                            sub,
                            &selected,
                            world_scale * 8.0,
                            [0.2, 0.6, 1.0, 1.0],
                            [1.0, 0.4, 0.1, 1.0],
                            [0.6, 0.6, 0.6, 1.0],
                        ));
                    }
                }
            }
            _ => {}
        }
        // Posisi overlay tool adalah (u, v) bidang; petakan ke dunia sedikit di
        // atas bidang agar tidak z-fighting dengan geometri sketsa.
        for v in &mut lines {
            let w = self
                .active_plane
                .to_world(DVec2::new(v.position[0] as f64, v.position[1] as f64), 0.01);
            v.position = [w.x, w.y, w.z];
        }
        lines
    }

    /// Terjemahkan interaksi egui menjadi event tool vektor.
    pub(crate) fn handle_vector_tool_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        cursor: DVec2,
        tol: f64,
    ) {
        let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
        let mods = VectorMods { shift, alt };
        let press = ui
            .input(|i| i.pointer.press_origin())
            .and_then(|pos| screen_to_plane_point(&self.camera, rect, pos, &self.active_plane))
            .unwrap_or(cursor);

        let mut events = Vec::with_capacity(2);
        if response.drag_started() {
            events.push(VectorPointer::Down(press));
        }
        if response.dragged() {
            events.push(VectorPointer::Drag(cursor));
        }
        if response.drag_stopped() {
            events.push(VectorPointer::Up(cursor));
        } else if response.clicked() {
            events.push(VectorPointer::Down(cursor));
            events.push(VectorPointer::Up(cursor));
        }

        let is_pen = self.tool == ToolKind::PenBezier;
        for ev in events {
            if is_pen {
                self.pen_pointer(ev, mods, tol);
            } else {
                self.node_edit_pointer(ev, mods, tol);
            }
        }

        if ui.ctx().memory(|m| m.focused().is_some()) {
            return;
        }
        if is_pen {
            if ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                self.pen_finish();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Backspace)) {
                self.pen_undo_node();
            }
        } else if !self.vector_state.node_selection.is_empty()
            && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        {
            self.node_edit_delete_selected();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::AppMode;
    use ducad_sketch::{PathSeg, Subpath};

    const TOL: f64 = 0.5;

    fn vector_app(tool: ToolKind) -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Vector);
        app.set_tool(tool);
        app
    }

    fn click(app: &mut DuCADApp, p: DVec2) -> Option<EntityId> {
        let m = VectorMods::default();
        let id = app.pen_pointer(VectorPointer::Down(p), m, TOL);
        app.pen_pointer(VectorPointer::Up(p), m, TOL);
        id
    }

    fn only_path(app: &DuCADApp) -> (EntityId, Subpath) {
        let paths: Vec<_> = app
            .sketch()
            .entities
            .iter()
            .filter_map(|(id, e)| match e {
                Entity::Path { subpaths, .. } => Some((id, subpaths[0].clone())),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 1, "tepat satu path");
        paths[0].clone()
    }

    #[test]
    fn pen_clicks_closing_on_start_commit_one_undo_step() {
        let mut app = vector_app(ToolKind::PenBezier);
        for p in [DVec2::ZERO, DVec2::new(20.0, 0.0), DVec2::new(20.0, 20.0)] {
            assert!(click(&mut app, p).is_none());
        }
        // Klik dalam toleransi node awal menutup path.
        let id = click(&mut app, DVec2::new(0.2, 0.1)).expect("path ter-commit");
        let (pid, sub) = only_path(&app);
        assert_eq!(pid, id);
        assert!(sub.closed);
        assert_eq!(sub.node_count(), 3);
        assert_eq!(app.sketch().style_of(id), app.vector_state.last_style);

        app.undo();
        assert!(
            app.sketch().entities.is_empty(),
            "satu undo membatalkan seluruh Pen"
        );
    }

    #[test]
    fn pen_drag_turns_pressed_node_into_one_smooth_node() {
        let mut app = vector_app(ToolKind::PenBezier);
        click(&mut app, DVec2::ZERO);
        let m = VectorMods::default();
        let node = DVec2::new(20.0, 0.0);
        app.pen_pointer(VectorPointer::Down(node), m, TOL);
        for x in [22.0, 24.0, 26.0] {
            app.pen_pointer(VectorPointer::Drag(DVec2::new(x, 5.0)), m, TOL);
        }
        app.pen_pointer(VectorPointer::Up(DVec2::new(26.0, 5.0)), m, TOL);
        click(&mut app, DVec2::new(40.0, 0.0));
        app.pen_finish().expect("path terbuka ter-commit");

        let (_, sub) = only_path(&app);
        assert_eq!(sub.node_count(), 3, "drag tidak menambah node per frame");
        match sub.segs[1] {
            PathSeg::Cubic { c1, .. } => assert_eq!(c1, DVec2::new(26.0, 5.0)),
            PathSeg::Line { .. } => panic!("node halus harus menghasilkan segmen kubik"),
        }
    }

    #[test]
    fn pen_backspace_drops_last_node_and_esc_needs_two_nodes() {
        let mut app = vector_app(ToolKind::PenBezier);
        click(&mut app, DVec2::ZERO);
        click(&mut app, DVec2::new(15.0, 0.0));
        assert!(app.pen_undo_node());
        assert!(
            app.pen_finish().is_none(),
            "satu node tidak menghasilkan path"
        );
        assert!(app.sketch().entities.is_empty());
    }

    fn app_with_square() -> (DuCADApp, EntityId) {
        let mut app = vector_app(ToolKind::NodeEdit);
        let sq = Subpath {
            start: DVec2::ZERO,
            segs: vec![
                PathSeg::Line {
                    end: DVec2::new(10.0, 0.0),
                },
                PathSeg::Line {
                    end: DVec2::new(10.0, 10.0),
                },
                PathSeg::Line {
                    end: DVec2::new(0.0, 10.0),
                },
            ],
            closed: true,
        };
        app.execute_sketch_command(Box::new(ducad_sketch::InsertEntities::new(
            "Pen",
            vec![Entity::path(vec![sq])],
        )));
        let id = app.sketch().entities.keys().next().unwrap();
        (app, id)
    }

    #[test]
    fn node_drag_updates_path_and_is_one_undo_step() {
        let (mut app, id) = app_with_square();
        let m = VectorMods::default();
        app.node_edit_pointer(VectorPointer::Down(DVec2::new(10.0, 10.0)), m, TOL);
        assert!(app.vector_state.node_selection.contains(&(id, 0, 2)));
        for d in 1..=5 {
            let p = DVec2::new(10.0 + d as f64, 10.0 + d as f64);
            app.node_edit_pointer(VectorPointer::Drag(p), m, TOL);
        }
        app.node_edit_pointer(VectorPointer::Up(DVec2::new(15.0, 15.0)), m, TOL);

        let (_, sub) = only_path(&app);
        assert_eq!(sub.node(2), Some(DVec2::new(15.0, 15.0)));
        app.undo();
        let (_, sub) = only_path(&app);
        assert_eq!(
            sub.node(2),
            Some(DVec2::new(10.0, 10.0)),
            "seluruh drag = satu undo"
        );
    }

    #[test]
    fn node_click_on_empty_space_clears_selection() {
        let (mut app, id) = app_with_square();
        let m = VectorMods::default();
        app.node_edit_pointer(VectorPointer::Down(DVec2::ZERO), m, TOL);
        app.node_edit_pointer(VectorPointer::Up(DVec2::ZERO), m, TOL);
        let shift = VectorMods { shift: true, ..m };
        app.node_edit_pointer(VectorPointer::Down(DVec2::new(10.0, 0.0)), shift, TOL);
        assert_eq!(app.vector_state.node_selection.len(), 2);
        assert!(app.vector_state.node_selection.contains(&(id, 0, 1)));
        app.node_edit_pointer(VectorPointer::Down(DVec2::new(50.0, 50.0)), m, TOL);
        assert!(app.vector_state.node_selection.is_empty());
    }

    #[test]
    fn node_delete_removes_selected_node_undoably() {
        let (mut app, id) = app_with_square();
        app.vector_state.select_single_node(id, 0, 1);
        app.node_edit_delete_selected();
        let (_, sub) = only_path(&app);
        assert_eq!(sub.node_count(), 3);
        app.undo();
        let (_, sub) = only_path(&app);
        assert_eq!(sub.node_count(), 4);
    }

    /// Regresi REVIEW-2026-09-24 #9: constraint strip bekerja pada seleksi node.
    #[test]
    fn node_constraint_strip_applies_horizontal_to_two_nodes() {
        use ducad_ui::ConstraintAction;
        let (mut app, id) = app_with_square();
        app.vector_state.select_single_node(id, 0, 0);
        assert_eq!(
            app.node_constraint_actions(),
            vec![ConstraintAction::ApplyFixed]
        );
        app.vector_state.toggle_node_selection(id, 0, 2);
        assert!(app
            .node_constraint_actions()
            .contains(&ConstraintAction::ApplyHorizontal));

        app.apply_node_constraint(ConstraintAction::ApplyHorizontal);
        assert_eq!(app.sketch().constraints.len(), 1);
        let (_, sub) = only_path(&app);
        let (a, b) = (sub.node(0).unwrap(), sub.node(2).unwrap());
        assert!(
            (a.y - b.y).abs() < 1e-6,
            "node 0 dan 2 kini sejajar horizontal"
        );
        app.undo();
        assert!(app.sketch().constraints.is_empty());
    }

    /// Tiga kotak 4×4 mm di (0,0), (10,5), (30,−3); semuanya terpilih.
    fn app_with_three_boxes() -> DuCADApp {
        let mut app = vector_app(ToolKind::Select);
        for (x, y) in [(0.0, 0.0), (10.0, 5.0), (30.0, -3.0)] {
            let sq = Subpath {
                start: DVec2::new(x, y),
                segs: vec![
                    PathSeg::Line {
                        end: DVec2::new(x + 4.0, y),
                    },
                    PathSeg::Line {
                        end: DVec2::new(x + 4.0, y + 4.0),
                    },
                    PathSeg::Line {
                        end: DVec2::new(x, y + 4.0),
                    },
                ],
                closed: true,
            };
            app.execute_sketch_command(Box::new(ducad_sketch::InsertEntities::new(
                "Pen",
                vec![Entity::path(vec![sq])],
            )));
        }
        app.selected = app.sketch().entities.keys().collect();
        app
    }

    fn bboxes(app: &DuCADApp) -> Vec<(DVec2, DVec2)> {
        app.sketch()
            .draw_order()
            .into_iter()
            .filter_map(|id| app.sketch().entities.get(id)?.bounding_box())
            .collect()
    }

    /// Regresi REVIEW-2026-09-24 #8: align lewat context bar = satu command.
    #[test]
    fn align_left_moves_all_to_selection_min_x_in_one_undo() {
        let mut app = app_with_three_boxes();
        app.apply_context_action(ducad_ui::ContextAction::AlignLeft);
        assert!(bboxes(&app).iter().all(|(min, _)| min.x.abs() < 1e-9));
        app.undo();
        assert!((bboxes(&app)[2].0.x - 30.0).abs() < 1e-9);
    }

    #[test]
    fn distribute_horizontal_spaces_centers_evenly() {
        let mut app = app_with_three_boxes();
        app.apply_context_action(ducad_ui::ContextAction::DistributeHorizontal);
        let mut cx: Vec<f64> = bboxes(&app)
            .iter()
            .map(|(a, b)| (a.x + b.x) * 0.5)
            .collect();
        cx.sort_by(f64::total_cmp);
        assert!(((cx[1] - cx[0]) - (cx[2] - cx[1])).abs() < 1e-9);
    }

    #[test]
    fn transform_dialog_sets_size_and_center_and_rejects_bad_input() {
        let mut app = app_with_three_boxes();
        app.apply_context_action(ducad_ui::ContextAction::TransformPrecise);
        let mut dialog = app
            .vector_state
            .transform_dialog
            .clone()
            .expect("dialog terbuka");
        assert_eq!(dialog.w, "34.000");

        dialog.w = "abc".into();
        assert!(app.apply_transform_dialog(&dialog).is_err());
        assert_eq!(
            app.sketch().entities.len(),
            3,
            "isian salah tidak mengubah apa pun"
        );

        dialog.x = "0".into();
        dialog.y = "0".into();
        dialog.w = "68".into();
        dialog.h = "18".into();
        app.apply_transform_dialog(&dialog).expect("isian sah");
        let (min, max) = bboxes(&app)
            .into_iter()
            .fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(a, b), (c, d)| {
                (a.min(c), b.max(d))
            });
        assert!(((max.x - min.x) - 68.0).abs() < 1e-6);
        assert!(((max.y - min.y) - 18.0).abs() < 1e-6);
        assert!(((min + max) * 0.5).length() < 1e-6);
        assert_eq!(app.selected.len(), 3, "seleksi mengikuti entitas hasil");
    }

    #[test]
    fn overlay_shows_selected_path_nodes_in_world_space() {
        let (mut app, id) = app_with_square();
        assert!(app.vector_overlay_lines(None, 0.1).is_empty());
        app.vector_state.select_single_node(id, 0, 0);
        let lines = app.vector_overlay_lines(None, 0.1);
        assert!(!lines.is_empty());
        assert!(lines
            .iter()
            .all(|v| v.position.iter().all(|c| c.is_finite())));
    }
}
