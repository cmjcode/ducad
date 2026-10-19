//! Bagian backend truck dari `crate::topo` (PLAN_MIGRASI_TRUCK 2.6):
//! enumerasi face & edge dari topologi logis (`super::topology`), dengan
//! arti tiap field sama dengan backend lama (≤ 0.5.x).

use glam::DVec3;
use solidcraft_kernel::FaceSurface;

use super::bridge::to_dv;
use super::topology::{LFace, Topology};
use crate::picking::face::SurfaceKind;
use crate::shape::KernelShape;
use crate::topo::{EdgeInfo, EdgeKind, FaceInfo};

/// Arah satuan dengan komponen non-nol pertama positif (sama dengan backend lama (≤ 0.5.x)).
fn canonical_dir(d: DVec3) -> DVec3 {
    let d = d.normalize_or_zero();
    for c in d.to_array() {
        if c.abs() > 1e-9 {
            return if c < 0.0 { -d } else { d };
        }
    }
    d
}

/// Daftar edge logis shape. Indeks sama dengan yang diterima
/// `fillet_edges_by_index`/`chamfer_edges_by_index`.
pub fn enumerate_edges(shape: &KernelShape) -> Vec<EdgeInfo> {
    shape
        .topology()
        .edges
        .iter()
        .enumerate()
        .map(|(index, e)| {
            let (start, end) = (e.start(), e.end());
            EdgeInfo {
                index,
                kind: e.kind,
                start: start.to_array(),
                end: end.to_array(),
                mid: e.mid().to_array(),
                length: e.length(),
                dir: (e.kind == EdgeKind::Line).then(|| canonical_dir(end - start).to_array()),
                radius: e.radius,
            }
        })
        .collect()
}

/// Sumbu (titik, arah satuan) face silinder/kerucut.
pub(crate) fn face_axis(face: &LFace) -> Option<(DVec3, DVec3)> {
    match face.surface? {
        FaceSurface::Cylinder { origin, axis, .. } => Some((to_dv(origin), to_dv(*axis))),
        FaceSurface::Cone { apex, axis, .. } => Some((to_dv(apex), to_dv(*axis))),
        _ => None,
    }
}

/// Radius permukaan analitik face. Kerucut: radius terbesar di batasnya
/// (backend lama (≤ 0.5.x) melaporkan radius referensi, yang untuk kerucut DUCAD = alasnya).
pub(crate) fn face_radius(topo: &Topology, f: usize) -> Option<f64> {
    let face = topo.faces.get(f)?;
    match face.surface? {
        FaceSurface::Cylinder { radius, .. } | FaceSurface::Sphere { radius, .. } => Some(radius),
        FaceSurface::Cone { apex, axis, .. } => {
            let (o, a) = (to_dv(apex), to_dv(*axis));
            face.edges
                .iter()
                .filter_map(|&e| topo.edges.get(e))
                .flat_map(|e| e.points.iter())
                .map(|p| {
                    let r = *p - o;
                    (r - a * r.dot(a)).length()
                })
                .max_by(f64::total_cmp)
        }
        _ => None,
    }
}

/// Normal keluar face logis: bidang → normal bidangnya; lengkung → di titik
/// tengah edge terpanjangnya (pusat massa silinder ada di sumbunya).
pub(crate) fn face_normal(topo: &Topology, f: usize) -> DVec3 {
    let Some(face) = topo.faces.get(f) else {
        return DVec3::ZERO;
    };
    if let Some(FaceSurface::Plane { normal, .. }) = face.surface {
        return to_dv(*normal);
    }
    let at = face
        .edges
        .iter()
        .filter_map(|&e| topo.edges.get(e))
        .max_by(|a, b| a.length().total_cmp(&b.length()))
        .map(|e| e.mid())
        .unwrap_or(face.centroid);
    topo.normal_near(f, at).unwrap_or(DVec3::ZERO)
}

/// Daftar face logis shape. Indeks sama dengan `KernelMesh::face_ranges`.
pub fn enumerate_faces(shape: &KernelShape) -> Vec<FaceInfo> {
    let topo = shape.topology();
    topo.faces
        .iter()
        .enumerate()
        .map(|(index, face)| {
            let axis = face_axis(face);
            let concave = axis.and_then(|(ap, ad)| {
                let [a, b, c] = topo.face_triangles(index).next()?;
                let p = (a + b + c) / 3.0;
                let n = (b - a).cross(c - a).normalize_or_zero();
                let rel = p - ap;
                let radial = (rel - ad * rel.dot(ad)).normalize_or_zero();
                Some(n.dot(radial) < 0.0)
            });
            let (mut lo, mut hi) = (face.centroid, face.centroid);
            for p in face
                .edges
                .iter()
                .filter_map(|&e| topo.edges.get(e))
                .flat_map(|e| e.points.iter())
            {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
            FaceInfo {
                index,
                kind: face.kind,
                centroid: face.centroid.to_array(),
                normal: face_normal(topo, index).to_array(),
                area: face.area,
                radius: match face.kind {
                    SurfaceKind::Cylinder | SurfaceKind::Cone | SurfaceKind::Sphere => {
                        face_radius(topo, index)
                    }
                    _ => None,
                },
                axis: axis.map(|(p, d)| (p.to_array(), d.to_array())),
                bbox: (lo.to_array(), hi.to_array()),
                edge_indices: face.edges.clone(),
                concave,
                boundary: topo
                    .face_boundary(index)
                    .into_iter()
                    .map(|p| p.to_array())
                    .collect(),
            }
        })
        .collect()
}
