//! Kueri geometri kecil yang dipakai modul bersama (`hlr`, `mass`,
//! `interference`) — satu-satunya titik sentuh OCCT mereka (PLAN_MIGRASI_TRUCK
//! F2.2). TIDAK mengambil `lock_kernel()`: pemanggil sudah memegangnya.

use glam::{vec3, Vec3};

use crate::hlr::AxialFace;
use crate::shape::KernelShape;

/// Polyline tiap edge B-rep (urutan `Shape::edges()`), koordinat dunia. Edge
/// tanpa aproksimasi diwakili dua titik ujungnya.
pub(crate) fn edge_polylines(shape: &KernelShape) -> Vec<Vec<Vec3>> {
    shape
        .inner()
        .edges()
        .map(|edge| {
            let pts: Vec<Vec3> = edge
                .approximation_segments()
                .map(|p| vec3(p.x as f32, p.y as f32, p.z as f32))
                .collect();
            if pts.len() >= 2 {
                pts
            } else {
                let (a, b) = (edge.start_point(), edge.end_point());
                vec![
                    vec3(a.x as f32, a.y as f32, a.z as f32),
                    vec3(b.x as f32, b.y as f32, b.z as f32),
                ]
            }
        })
        .collect()
}

/// Face silinder/kerucut beserta sumbunya dan polyline edge batasnya.
pub(crate) fn axial_faces(shape: &KernelShape) -> Vec<AxialFace> {
    shape
        .inner()
        .faces()
        .filter_map(|face| {
            let (pt, dir) = face.cylinder_or_cone_axis()?;
            let boundary = face
                .edges()
                .map(|edge| {
                    edge.approximation_segments()
                        .map(|p| vec3(p.x as f32, p.y as f32, p.z as f32))
                        .collect()
                })
                .collect();
            Some(AxialFace {
                origin: vec3(pt.x as f32, pt.y as f32, pt.z as f32),
                dir: vec3(dir.x as f32, dir.y as f32, dir.z as f32),
                boundary,
            })
        })
        .collect()
}

/// Jarak minimum eksak (`BRepExtrema_DistShapeShape`) dan titik terdekatnya.
pub(crate) fn min_distance(a: &KernelShape, b: &KernelShape) -> Option<(f64, [f64; 3], [f64; 3])> {
    let (d, pa, pb) = a.inner().min_distance_to(b.inner())?;
    Some((d, pa.to_array(), pb.to_array()))
}

/// Volume bertanda, pusat massa, dan tensor inersia terhadap origin
/// (`BRepGProp`, densitas 1).
pub(crate) fn volume_properties(shape: &KernelShape) -> (f64, [f64; 3], [[f64; 3]; 3]) {
    let (v, c, i) = shape.inner().volume_properties();
    (v, c.to_array(), i)
}
