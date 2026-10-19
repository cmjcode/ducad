//! Kueri geometri kecil untuk modul bersama (`hlr`, `mass`, `interference`)
//! — padanan `backend/occt/query.rs` di atas SolidCraft.

use glam::{vec3, DVec3, Vec3};
use solidcraft_kernel::DistanceShape;

use super::bridge::to_dv;
use super::topo::face_axis;
use crate::hlr::AxialFace;
use crate::shape::KernelShape;

fn f32v(p: &DVec3) -> Vec3 {
    vec3(p.x as f32, p.y as f32, p.z as f32)
}

/// Polyline tiap edge logis, koordinat dunia.
pub(crate) fn edge_polylines(shape: &KernelShape) -> Vec<Vec<Vec3>> {
    shape
        .topology()
        .edges
        .iter()
        .map(|e| e.points.iter().map(f32v).collect())
        .collect()
}

/// Face silinder/kerucut logis beserta sumbunya dan polyline edge batasnya.
pub(crate) fn axial_faces(shape: &KernelShape) -> Vec<AxialFace> {
    let topo = shape.topology();
    topo.faces
        .iter()
        .filter_map(|face| {
            let (origin, dir) = face_axis(face)?;
            Some(AxialFace {
                origin: f32v(&origin),
                dir: f32v(&dir),
                boundary: face
                    .edges
                    .iter()
                    .filter_map(|&e| topo.edges.get(e))
                    .map(|e| e.points.iter().map(f32v).collect())
                    .collect(),
            })
        })
        .collect()
}

/// Jarak minimum (permukaan eksak bila bisa, selain itu mesh) dan titik
/// terdekatnya (`solidcraft_kernel::body_distance`).
pub(crate) fn min_distance(a: &KernelShape, b: &KernelShape) -> Option<(f64, [f64; 3], [f64; 3])> {
    let (ba, bb) = (a.body()?, b.body()?);
    let chord = |s: f64| (s * 1e-4).max(1e-4);
    let (ca, cb) = (chord(ba.size()), chord(bb.size()));
    let found = solidcraft_kernel::guard("min distance", || {
        let (ma, mb) = (ba.tessellate(ca)?, bb.tessellate(cb)?);
        let (sa, sb) = (
            DistanceShape::new(ba, &ma, ca),
            DistanceShape::new(bb, &mb, cb),
        );
        solidcraft_kernel::body_distance(&sa, &sb, f64::INFINITY)
    })
    .ok()
    .flatten()?;
    Some((
        found.distance,
        to_dv(found.from).to_array(),
        to_dv(found.to).to_array(),
    ))
}

/// Volume, pusat massa, dan tensor inersia terhadap origin (densitas 1)
/// dari tesselasi halus (teorema divergensi per tetrahedron origin–segitiga).
/// Elemen luar-diagonal bertanda negatif (`I_xy = −∫xy dV`), sama dengan
/// konvensi `MassProperties::inertia_origin`.
pub(crate) fn volume_properties(shape: &KernelShape) -> (f64, [f64; 3], [[f64; 3]; 3]) {
    let empty = (0.0, [0.0; 3], [[0.0; 3]; 3]);
    let Some(body) = shape.body() else {
        return empty;
    };
    let tol = (body.size() * 5e-5).max(1e-5);
    let Ok(mesh) = solidcraft_kernel::guard("mass properties", || body.tessellate_with(tol, true))
    else {
        return empty;
    };
    let (mut v6, mut first, mut s) = (0.0f64, DVec3::ZERO, [[0.0f64; 3]; 3]);
    for t in &mesh.triangles {
        let Some([a, b, c]) = mesh.tri(t) else {
            continue;
        };
        let (a, b, c) = (to_dv(a), to_dv(b), to_dv(c));
        let det = a.dot(b.cross(c));
        v6 += det;
        first += (a + b + c) * det;
        // ∫ x xᵀ atas tetrahedron (0, a, b, c) = det/120 · (Σ p pᵀ + s sᵀ).
        let (p, sum) = ([a, b, c], a + b + c);
        for (i, row) in s.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell +=
                    det / 120.0 * (p.iter().map(|q| q[i] * q[j]).sum::<f64>() + sum[i] * sum[j]);
            }
        }
    }
    let volume = v6 / 6.0;
    if volume.abs() < 1e-12 {
        return empty;
    }
    let centroid = first / (4.0 * v6);
    let trace = s[0][0] + s[1][1] + s[2][2];
    let mut inertia = [[0.0; 3]; 3];
    for (i, row) in inertia.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = if i == j { trace - s[i][j] } else { -s[i][j] };
        }
    }
    (volume, centroid.to_array(), inertia)
}
