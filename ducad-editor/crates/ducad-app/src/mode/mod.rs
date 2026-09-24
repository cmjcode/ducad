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
}

impl AppMode {
    /// Mengembalikan true jika dalam mode 2D (Sketch atau Vektor).
    pub fn is_2d(self) -> bool {
        matches!(self, AppMode::Sketch | AppMode::Vector)
    }

    /// Mengembalikan true jika dalam mode 3D solid.
    pub fn is_3d(self) -> bool {
        matches!(self, AppMode::Solid)
    }
}
