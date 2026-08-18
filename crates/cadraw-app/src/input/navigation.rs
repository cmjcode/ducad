use eframe::egui;

use crate::app::CadrawApp;
use crate::viewport::pick_inactive_plane_at_cursor;

impl CadrawApp {
    pub fn handle_navigation(
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
            && !modifiers.shift
            && !self.extruding_from_gizmo
            && !self.extruding_face_from_gizmo)
            || (response.dragged_by(egui::PointerButton::Middle) && !modifiers.shift);
        let panning = response.dragged_by(egui::PointerButton::Secondary)
            || (modifiers.shift
                && !self.extruding_from_gizmo
                && !self.extruding_face_from_gizmo
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
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let factor = 1.0 - scroll * 0.0015;
                self.camera.zoom(factor);
            }
        }

        if let Some(touch) = ui.input(|i| i.multi_touch()) {
            if modifiers.shift {
                self.camera
                    .pan(touch.translation_delta.x, touch.translation_delta.y, rect.height());
            } else {
                self.camera
                    .orbit(touch.translation_delta.x, touch.translation_delta.y);
            }
            self.camera.zoom(touch.zoom_delta);
        }
    }

    pub fn handle_plane_activation(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
    ) -> bool {
        if response.clicked() && ui.input(|i| i.modifiers.command) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(kind) =
                    pick_inactive_plane_at_cursor(&self.camera, rect, pos, self.active_plane.kind)
                {
                    self.activate_plane_from_viewport(kind);
                    return true;
                }
            }
        }

        const TAP_MAX_SECS: f64 = 0.3;
        const TAP_MOVE_TOLERANCE: f32 = 10.0;

        match ui.input(|i| i.multi_touch()) {
            Some(t) if t.num_touches == 2 => {
                self.two_finger_tap_press = Some(t);
            }
            _ => {
                if let Some(t) = self.two_finger_tap_press.take() {
                    let now = ui.input(|i| i.time);
                    let elapsed = now - t.start_time;
                    let drift = t.center_pos.distance(t.start_pos);
                    if elapsed <= TAP_MAX_SECS && drift <= TAP_MOVE_TOLERANCE {
                        if let Some(kind) = pick_inactive_plane_at_cursor(
                            &self.camera,
                            rect,
                            t.center_pos,
                            self.active_plane.kind,
                        ) {
                            self.activate_plane_from_viewport(kind);
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}
