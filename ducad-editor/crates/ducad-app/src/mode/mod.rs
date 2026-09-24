//! Mode aplikasi DuCAD: Sketch 2D, Vector 2D, dan Solid 3D (M2/M6).

/// Mode interaksi kanvas aktif dalam DuCAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    /// Mode 2D CAD Sketch: garis, lingkaran, constraint solver parametrik.
    #[default]
    Sketch,
    /// Mode 2D Vektor: kurva Bézier, edit node, boolean path, gaya/layer CorelDraw.
    Vector,
    /// Mode 3D Solid: pemodelan B-rep, extrude, revolve, dsb.
    Solid,
    /// Mode Sketsa Tinta Bebas (Concepts-like): kuas bertekanan, layer, kanvas tak hingga.
    Ink,
}

impl AppMode {
    /// Mengembalikan true jika dalam mode 2D (Sketch, Vektor, atau Sketsa Tinta).
    pub fn is_2d(self) -> bool {
        matches!(self, AppMode::Sketch | AppMode::Vector | AppMode::Ink)
    }

    /// Mengembalikan true jika dalam mode 3D solid.
    pub fn is_3d(self) -> bool {
        matches!(self, AppMode::Solid)
    }

    /// Mengembalikan true jika dalam mode Sketsa Tinta.
    pub fn is_ink(self) -> bool {
        matches!(self, AppMode::Ink)
    }

    /// Indeks stabil untuk tabel per mode (mis. tool terakhir).
    pub fn index(self) -> usize {
        match self {
            AppMode::Sketch => 0,
            AppMode::Vector => 1,
            AppMode::Solid => 2,
            AppMode::Ink => 3,
        }
    }

    /// Label mode untuk UI.
    pub fn label(self) -> &'static str {
        match self {
            AppMode::Sketch => "Mode Sketsa CAD",
            AppMode::Vector => "Mode Vektor",
            AppMode::Solid => "Mode 3D Solid",
            AppMode::Ink => "Mode Sketsa Tinta",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppMode;
    use crate::app::DuCADApp;
    use crate::types::ToolKind;
    use ducad_render::CameraMode;

    #[test]
    fn set_mode_restores_last_tool_per_mode() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Vector);
        app.set_tool(ToolKind::PenBezier);
        app.set_app_mode(AppMode::Solid);
        assert_eq!(
            app.tool,
            ToolKind::Select,
            "mode baru tanpa riwayat → Select"
        );
        app.set_tool(ToolKind::Extrude);
        app.set_app_mode(AppMode::Vector);
        assert_eq!(app.tool, ToolKind::PenBezier);
        app.set_app_mode(AppMode::Solid);
        assert_eq!(app.tool, ToolKind::Extrude);
    }

    #[test]
    fn set_mode_switches_camera_mode() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Vector);
        assert!(matches!(app.camera.mode(), CameraMode::Ortho2D { .. }));
        assert!(app.is_sketching, "tool vektor butuh mode sketching");
        app.set_app_mode(AppMode::Solid);
        assert!(matches!(app.camera.mode(), CameraMode::Orbit));
        app.set_app_mode(AppMode::Ink);
        assert!(matches!(app.camera.mode(), CameraMode::Ortho2D { .. }));
    }

    #[test]
    fn set_mode_is_reachable_from_palette() {
        let app = DuCADApp::new_for_test();
        let actions = app.palette_actions();
        for mode in [
            AppMode::Vector,
            AppMode::Ink,
            AppMode::Sketch,
            AppMode::Solid,
        ] {
            assert!(
                actions.iter().any(|(label, _, _)| label == mode.label()),
                "{} harus ada di command palette",
                mode.label()
            );
        }
    }
}
