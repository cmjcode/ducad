//! Bagian backend truck dari `crate::picking::face` (PLAN_MIGRASI_TRUCK 2.6):
//! ray dilempar ke mesh tampilan (segitiganya sudah dikelompokkan per face
//! logis), lalu detail face diambil dari topologi logis.

use glam::DVec3;
use solidcraft_kernel::FaceSurface;

use super::bridge::to_dv;
use super::topo::{face_axis, face_radius};
use super::topology::Topology;
use crate::picking::face::{FaceHit, RadialAxis, SurfaceKind};
use crate::picking::ray::PickRay;
use crate::shape::KernelShape;

/// Möller–Trumbore: jarak `t` (dalam satuan `dir`) ray ke segitiga.
fn ray_triangle(o: DVec3, d: DVec3, [a, b, c]: [DVec3; 3]) -> Option<f64> {
    let (e1, e2) = (b - a, c - a);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-14 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t >= 0.0).then_some(t)
}

/// Face logis terdekat yang kena ray, titik hit, dan normal segitiganya.
pub(crate) fn resolve_face_hit(topo: &Topology, ray: PickRay) -> Option<(usize, DVec3, DVec3)> {
    let (o, d) = (ray.origin_vec(), ray.dir_vec());
    if d.length_squared() < 1e-18 {
        return None;
    }
    let mut best: Option<(f64, usize, [DVec3; 3])> = None;
    for f in 0..topo.faces.len() {
        for tri in topo.face_triangles(f) {
            if let Some(t) = ray_triangle(o, d, tri) {
                if best.as_ref().is_none_or(|(bt, ..)| t < *bt) {
                    best = Some((t, f, tri));
                }
            }
        }
    }
    best.map(|(t, f, [a, b, c])| (f, o + d * t, (b - a).cross(c - a).normalize_or_zero()))
}

/// Cast `ray` ke `shape`, kembalikan detail face terdekat.
pub fn pick_face_details(shape: &KernelShape, ray: PickRay) -> Option<FaceHit> {
    let topo = shape.topology();
    let (f, hit, tri_normal) = resolve_face_hit(topo, ray)?;
    let face = topo.faces.get(f)?;
    let dir = ray.dir_vec();
    let toward_viewer = |n: DVec3| if n.dot(dir) > 0.0 { -n } else { n };
    let boundary = topo.face_boundary(f);
    let (normal, centroid) = match (face.kind, face.surface) {
        (SurfaceKind::Plane, surface) => {
            let n = match surface {
                Some(FaceSurface::Plane { normal, .. }) => to_dv(*normal),
                _ => tri_normal,
            };
            let c = if boundary.is_empty() {
                face.centroid
            } else {
                boundary.iter().fold(DVec3::ZERO, |acc, p| acc + *p) / boundary.len() as f64
            };
            (toward_viewer(n), c)
        }
        (SurfaceKind::Sphere, Some(FaceSurface::Sphere { center, .. })) => {
            (toward_viewer(tri_normal), to_dv(center))
        }
        _ => (toward_viewer(tri_normal), face.centroid),
    };
    let axis = face_axis(face);
    let pull_dir = match face.kind {
        SurfaceKind::Plane => normal,
        SurfaceKind::Cylinder | SurfaceKind::Cone => axis
            .and_then(|(o, a)| {
                let radial = hit - (o + a * (hit - o).dot(a));
                (radial.length() > 1e-9).then(|| radial.normalize())
            })
            .unwrap_or(normal),
        SurfaceKind::Sphere => match face.surface {
            Some(FaceSurface::Sphere { center, .. }) => {
                let r = (hit - to_dv(center)).normalize_or_zero();
                if r == DVec3::ZERO {
                    normal
                } else {
                    toward_viewer(r)
                }
            }
            _ => normal,
        },
        SurfaceKind::Torus | SurfaceKind::Other => normal,
    };
    let radial_axis = match (face.kind, face.surface) {
        (SurfaceKind::Cylinder | SurfaceKind::Cone, _) => axis.map(|(o, d)| RadialAxis {
            origin: (o.x, o.y, o.z),
            dir: (d.x, d.y, d.z),
        }),
        (SurfaceKind::Sphere, Some(FaceSurface::Sphere { center, .. })) => Some(RadialAxis {
            origin: (center.x, center.y, center.z),
            dir: (0.0, 0.0, 0.0),
        }),
        _ => None,
    };
    let t = |v: DVec3| (v.x, v.y, v.z);
    Some(FaceHit {
        hit_point: t(hit),
        centroid: t(centroid),
        normal: t(normal),
        surface_kind: face.kind,
        pull_dir: t(pull_dir),
        boundary_points: boundary.into_iter().map(t).collect(),
        face_index: Some(f),
        surface_radius: match face.kind {
            SurfaceKind::Cylinder | SurfaceKind::Cone | SurfaceKind::Sphere => face_radius(topo, f),
            _ => None,
        },
        radial_axis,
    })
}

/// Cast `ray` ke `shape`, kembalikan titik hit face TERDEKAT (kalau ada).
pub fn pick_face(shape: &KernelShape, ray: PickRay) -> Option<(f64, f64, f64)> {
    resolve_face_hit(shape.topology(), ray).map(|(_, p, _)| (p.x, p.y, p.z))
}
