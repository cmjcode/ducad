//! `PlaneFrame` — bidang sketch versi engine (f64, tanpa `ducad-render`).
//!
//! Nilai tiga bidang standar HARUS identik dengan `ducad_render::SketchPlane`
//! (`ducad-render/src/plane.rs`), termasuk normal Front = −Y yang memang
//! BUKAN `u × v`. GUI mengonversi `SketchPlane` ke `PlaneFrame` lewat
//! `ducad_app::document::plane_frame_from` tanpa menghitung ulang normal.

use glam::{DVec2, DVec3};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlaneFrame {
    pub origin: [f64; 3],
    /// Satuan.
    pub u_axis: [f64; 3],
    /// Satuan, tegak lurus `u_axis`.
    pub v_axis: [f64; 3],
    /// Satuan; TIDAK selalu `u × v` (lihat [`PlaneFrame::front`]).
    pub normal: [f64; 3],
}

impl Default for PlaneFrame {
    fn default() -> Self {
        Self::top()
    }
}

impl PlaneFrame {
    /// Bidang Top (XY): u = +X, v = +Y, normal = +Z.
    pub fn top() -> Self {
        Self {
            origin: [0.0, 0.0, 0.0],
            u_axis: [1.0, 0.0, 0.0],
            v_axis: [0.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        }
    }

    /// Bidang Front (XZ): u = +X, v = +Z, normal = −Y.
    pub fn front() -> Self {
        Self {
            origin: [0.0, 0.0, 0.0],
            u_axis: [1.0, 0.0, 0.0],
            v_axis: [0.0, 0.0, 1.0],
            normal: [0.0, -1.0, 0.0],
        }
    }

    /// Bidang Right (YZ): u = +Y, v = +Z, normal = +X.
    pub fn right() -> Self {
        Self {
            origin: [0.0, 0.0, 0.0],
            u_axis: [0.0, 1.0, 0.0],
            v_axis: [0.0, 0.0, 1.0],
            normal: [1.0, 0.0, 0.0],
        }
    }

    /// Terjemahkan `PlaneRef` menjadi geometri. Datum dicari berdasarkan id
    /// di `datums`; `None` bila id datum tidak dikenal.
    pub fn from_plane_ref(p: ducad_sketch::PlaneRef, datums: &[(u32, PlaneFrame)]) -> Option<Self> {
        match p {
            ducad_sketch::PlaneRef::Top => Some(Self::top()),
            ducad_sketch::PlaneRef::Front => Some(Self::front()),
            ducad_sketch::PlaneRef::Right => Some(Self::right()),
            ducad_sketch::PlaneRef::Datum(id) => {
                datums.iter().find(|(d, _)| *d == id).map(|(_, f)| *f)
            }
        }
    }

    pub fn origin_v(&self) -> DVec3 {
        DVec3::from_array(self.origin)
    }

    pub fn u_v(&self) -> DVec3 {
        DVec3::from_array(self.u_axis)
    }

    pub fn v_v(&self) -> DVec3 {
        DVec3::from_array(self.v_axis)
    }

    pub fn normal_v(&self) -> DVec3 {
        DVec3::from_array(self.normal)
    }

    /// `origin + u*x + v*y`
    pub fn to_world(&self, p: DVec2) -> DVec3 {
        self.origin_v() + self.u_v() * p.x + self.v_v() * p.y
    }

    /// Sama dengan `SketchPlane::to_world_f64`: titik bidang + offset
    /// sepanjang normal, dalam bentuk larik.
    pub fn to_world_f64(&self, p: (f64, f64), offset: f64) -> [f64; 3] {
        (self.to_world(DVec2::new(p.0, p.1)) + self.normal_v() * offset).to_array()
    }

    /// Proyeksi titik dunia ke koordinat (u, v) bidang.
    pub fn to_local(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin_v();
        DVec2::new(d.dot(self.u_v()), d.dot(self.v_v()))
    }

    /// Bidang di atas sebuah face planar: origin = centroid, normal = normal
    /// face, u = proyeksi +X dunia ke bidang (bila panjangnya < 1e-6 pakai
    /// +Y dunia), dinormalisasi, v = normal × u.
    pub fn on_face(centroid: [f64; 3], normal: [f64; 3]) -> Self {
        let n = DVec3::from_array(normal).normalize_or_zero();
        let project = |w: DVec3| w - n * w.dot(n);
        let mut u = project(DVec3::X);
        if u.length() < 1e-6 {
            u = project(DVec3::Y);
        }
        let u = u.normalize_or_zero();
        let v = n.cross(u).normalize_or_zero();
        Self {
            origin: centroid,
            u_axis: u.to_array(),
            v_axis: v.to_array(),
            normal: n.to_array(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close3(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() <= tol)
    }

    #[test]
    fn standard_planes_match_convention_table() {
        let t = PlaneFrame::top();
        assert_eq!(t.origin, [0.0, 0.0, 0.0]);
        assert_eq!(
            (t.u_axis, t.v_axis, t.normal),
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
        );
        let f = PlaneFrame::front();
        assert_eq!(f.origin, [0.0, 0.0, 0.0]);
        assert_eq!(
            (f.u_axis, f.v_axis, f.normal),
            ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0])
        );
        let r = PlaneFrame::right();
        assert_eq!(r.origin, [0.0, 0.0, 0.0]);
        assert_eq!(
            (r.u_axis, r.v_axis, r.normal),
            ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0])
        );
    }

    #[test]
    fn to_local_inverts_to_world() {
        let p = DVec2::new(12.5, -7.25);
        for plane in [PlaneFrame::top(), PlaneFrame::front(), PlaneFrame::right()] {
            let back = plane.to_local(plane.to_world(p));
            assert!((back - p).length() < 1e-12, "{plane:?}: {back:?}");
        }
    }

    #[test]
    fn on_face_top_uses_world_x() {
        let f = PlaneFrame::on_face([0.0, 0.0, 8.0], [0.0, 0.0, 1.0]);
        assert!(close3(f.u_axis, [1.0, 0.0, 0.0], 1e-12));
        assert!(close3(f.v_axis, [0.0, 1.0, 0.0], 1e-12));
        assert_eq!(f.origin, [0.0, 0.0, 8.0]);
    }

    #[test]
    fn on_face_falls_back_to_world_y() {
        let f = PlaneFrame::on_face([3.0, 1.0, 2.0], [1.0, 0.0, 0.0]);
        assert!(close3(f.u_axis, [0.0, 1.0, 0.0], 1e-12));
        let v = DVec3::from_array(f.normal).cross(DVec3::from_array(f.u_axis));
        assert!(close3(f.v_axis, v.to_array(), 1e-12));
    }

    #[test]
    fn from_plane_ref_resolves_datum() {
        let d = PlaneFrame::on_face([0.0, 0.0, 5.0], [0.0, 0.0, 1.0]);
        assert_eq!(
            PlaneFrame::from_plane_ref(ducad_sketch::PlaneRef::Datum(7), &[(7, d)]),
            Some(d)
        );
        assert_eq!(
            PlaneFrame::from_plane_ref(ducad_sketch::PlaneRef::Datum(8), &[(7, d)]),
            None
        );
        assert_eq!(
            PlaneFrame::from_plane_ref(ducad_sketch::PlaneRef::Front, &[]),
            Some(PlaneFrame::front())
        );
    }
}
