use glam::DVec3;
use opencascade::primitives::{Face, Shape};

use crate::lock_kernel;
use crate::picking::ray::{point_in_polygon_2d, PickRay};
use crate::shape::KernelShape;

/// Toleransi geometris `BRepIntCurveSurface_Inter` untuk face-picking
/// interaktif — lebih longgar dari default upstream (`0.0001`, lihat
/// `vendor/README.md`). TERBUKTI (test terisolasi) TIDAK CUKUP sendirian
/// utk kasus ray oblique pada wajah sweep/samping (root cause BUKAN
/// toleransi — lihat `resolve_planar_face_along_ray_fallback` di bawah)
/// tapi tetap dipertahankan sebagai margin aman kecil, tidak merugikan.
pub(crate) const FACE_PICK_TOLERANCE_MM: f64 = 0.01;

/// Tipe permukaan geometris analitik OCCT di balik sebuah `Face`, hasil
/// klasifikasi `Face::surface_kind()` (nama kelas dinamis C++ `Geom_*`).
/// Fase 1: sekadar deteksi/label — belum dipakai fitur apa pun, disiapkan
/// utk fitur mendatang yang berperilaku beda tergantung tipe face (mis.
/// deteksi smart boolean, hint UI khusus silinder/bola).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    /// Permukaan analitik lain (mis. torus non-standar) atau permukaan
    /// bebas/non-analitik (mis. B-Spline, hasil loft/sweep).
    Other,
}

impl From<&str> for SurfaceKind {
    /// Petakan nama kelas dinamis OCCT (keluaran `Face::surface_kind()`)
    /// ke varian enum. Nama yang tidak dikenali (termasuk permukaan
    /// bebas/non-analitik) jatuh ke `Other`, bukan error — klasifikasi ini
    /// sifatnya informasional, tidak boleh gagal.
    fn from(dynamic_type_name: &str) -> Self {
        match dynamic_type_name {
            "Geom_Plane" => SurfaceKind::Plane,
            "Geom_CylindricalSurface" => SurfaceKind::Cylinder,
            "Geom_ConicalSurface" => SurfaceKind::Cone,
            "Geom_SphericalSurface" => SurfaceKind::Sphere,
            "Geom_ToroidalSurface" => SurfaceKind::Torus,
            _ => SurfaceKind::Other,
        }
    }
}

/// Informasi hit face hasil picking 3D (titik hit, titik pusat centroid, dan normal satuan keluar).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceHit {
    /// Titik hit langsung pada permukaan face (x, y, z)
    pub hit_point: (f64, f64, f64),
    /// Titik pusat / centroid aproksimasi face (x, y, z)
    pub centroid: (f64, f64, f64),
    /// Vektor normal satuan yang mengarah ke luar (outward normal) (x, y, z)
    pub normal: (f64, f64, f64),
    /// Tipe permukaan geometris analitik di balik face yang terpilih.
    pub surface_kind: SurfaceKind,
    /// Arah satuan yang dipakai gizmo push/pull (CADRAW Fase 4) — TIDAK
    /// selalu sama dengan `normal`:
    /// - `Plane`: identik dengan `normal` (normal Newell, perilaku lama).
    /// - `Cylinder`/`Cone`: arah RADIAL di `hit_point`, yaitu
    ///   `(hit_point − proyeksi hit_point ke garis sumbu).normalize()` —
    ///   inilah arah geometris yang benar-benar mengubah radius saat
    ///   di-drag (lihat dispatch `extrude_face`/`offset_on_face`), bukan
    ///   normal permukaan lokal di titik itu (yang juga radial tapi
    ///   Newell's method di atas boundary loop TIDAK menghitungnya per
    ///   titik — nilainya konstan per-face, salah untuk radial drag).
    /// - `Sphere`: `(hit_point − centroid).normalize()` — `centroid` bola
    ///   penuh (`Face::center_of_mass`) SECARA MATEMATIS persis pusat bola
    ///   (centroid luas permukaan bola simetris = pusatnya).
    /// - `Torus`/`Other`: fallback ke `normal` (belum ada rumus arah
    ///   push/pull khusus utk tipe ini).
    pub pull_dir: (f64, f64, f64),
}

impl FaceHit {
    /// Titik anchor gizmo push/pull (CADRAW Fase 8 lanjutan) — TIDAK selalu
    /// `centroid`:
    /// - `Plane`: `centroid`, sama seperti perilaku lama (face datar
    ///   biasanya punya region kecil, centroid tetap dekat permukaan).
    /// - selain `Plane` (`Cylinder`/`Cone`/`Sphere`/`Torus`/`Other`):
    ///   `hit_point` — utk selimut silinder/bola penuh, `centroid` Newell
    ///   jatuh di SUMBU (silinder) atau PUSAT bola (bola), yaitu di DALAM
    ///   material. Handle gizmo yg diletakkan di `centroid + pull_dir·18`
    ///   pun ikut terkubur utk radius > 18 mm — tak terlihat & tak bisa
    ///   di-drag. `hit_point` selalu ada DI PERMUKAAN, jadi anchor aman
    ///   utk semua radius.
    pub fn gizmo_anchor(&self) -> (f64, f64, f64) {
        if self.surface_kind == SurfaceKind::Plane {
            self.centroid
        } else {
            self.hit_point
        }
    }
}

/// Face terdekat (dari `ray.origin`) yang kena `ray`.
pub(crate) fn resolve_face_along_ray(shape: &Shape, ray: PickRay) -> Option<(Face, DVec3)> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    let occt_hit = shape
        .faces_along_ray_with_tolerance(origin, dir, FACE_PICK_TOLERANCE_MM)
        .into_iter()
        .min_by(|(_, p1), (_, p2)| {
            (*p1 - origin)
                .length_squared()
                .partial_cmp(&(*p2 - origin).length_squared())
                .expect("titik hit faces_along_ray tidak boleh NaN")
        });
    if occt_hit.is_some() {
        return occt_hit;
    }
    resolve_planar_face_along_ray_fallback(shape, ray)
}

/// Fallback ray-vs-poligon planar murni Rust — dipanggil HANYA saat jalur
/// OCCT (`faces_along_ray_with_tolerance`) di atas kosong (lihat dokumentasi
/// root-cause di `resolve_face_along_ray`).
pub(crate) fn resolve_planar_face_along_ray_fallback(shape: &Shape, ray: PickRay) -> Option<(Face, DVec3)> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    if dir.length_squared() < 1e-18 {
        return None;
    }

    let mut best: Option<(Face, DVec3, f64)> = None;
    for face in shape.faces() {
        let pts = chain_face_boundary_points(&face);
        if pts.len() < 3 {
            continue;
        }
        let centroid = pts.iter().fold(DVec3::ZERO, |acc, p| acc + *p) / (pts.len() as f64);

        // Newell's method — SAMA persis dgn `compute_face_normal_and_centroid`.
        let mut normal = DVec3::ZERO;
        for i in 0..pts.len() {
            let j = (i + 1) % pts.len();
            normal.x += (pts[i].y - pts[j].y) * (pts[i].z + pts[j].z);
            normal.y += (pts[i].z - pts[j].z) * (pts[i].x + pts[j].x);
            normal.z += (pts[i].x - pts[j].x) * (pts[i].y + pts[j].y);
        }
        let normal_len = normal.length();
        if normal_len < 1e-9 {
            continue;
        }
        let unit_normal = normal / normal_len;

        // Cek koplanar — wajah melengkung (silinder/revolve) deviasinya
        // BESAR dari bidang best-fit, dilewati (sudah benar via OCCT).
        let max_deviation = pts
            .iter()
            .map(|p| (*p - centroid).dot(unit_normal).abs())
            .fold(0.0_f64, f64::max);
        if max_deviation > 1e-3 {
            continue;
        }

        // Ray-plane intersection.
        let denom = dir.dot(unit_normal);
        if denom.abs() < 1e-9 {
            continue; // ray sejajar bidang wajah
        }
        let t = (centroid - origin).dot(unit_normal) / denom;
        if t <= 0.0 {
            continue; // perpotongan di belakang origin ray
        }
        let hit_point = origin + dir * t;

        // Point-in-polygon: proyeksi ke basis 2D bidang wajah.
        let u_axis = (pts[0] - centroid).normalize_or_zero();
        if u_axis == DVec3::ZERO {
            continue;
        }
        let v_axis = unit_normal.cross(u_axis).normalize_or_zero();
        let to_2d = |p: DVec3| -> (f64, f64) {
            let d = p - centroid;
            (d.dot(u_axis), d.dot(v_axis))
        };
        let poly2d: Vec<(f64, f64)> = pts.iter().map(|p| to_2d(*p)).collect();
        if !point_in_polygon_2d(to_2d(hit_point), &poly2d) {
            continue;
        }

        let dist_sq = (hit_point - origin).length_squared();
        if best.as_ref().is_none_or(|(_, _, d)| dist_sq < *d) {
            best = Some((face, hit_point, dist_sq));
        }
    }
    best.map(|(face, point, _)| (face, point))
}

/// Susun titik tepi sebuah `Face` jadi LOOP TERSAMBUNG berdasarkan
/// konektivitas titik ujung.
pub(crate) fn chain_face_boundary_points(face: &Face) -> Vec<DVec3> {
    let edge_pointlists: Vec<Vec<DVec3>> = face
        .edges()
        .map(|edge| edge.approximation_segments().collect::<Vec<DVec3>>())
        .filter(|pts| pts.len() >= 2)
        .collect();
    if edge_pointlists.is_empty() {
        return Vec::new();
    }
    if edge_pointlists.len() == 1 {
        return edge_pointlists.into_iter().next().expect("sudah dicek len()==1");
    }

    const EPS: f64 = 1e-6;
    let mut used = vec![false; edge_pointlists.len()];
    let mut chain = edge_pointlists[0].clone();
    used[0] = true;
    let mut remaining = edge_pointlists.len() - 1;
    while remaining > 0 {
        let tail = *chain.last().expect("chain tidak pernah kosong setelah inisialisasi");
        let mut found = false;
        for (i, pts) in edge_pointlists.iter().enumerate() {
            if used[i] {
                continue;
            }
            let start = pts[0];
            let end = *pts.last().expect("edge_pointlists sudah difilter len()>=2");
            if (start - tail).length_squared() < EPS {
                chain.extend(pts.iter().skip(1).copied());
                used[i] = true;
                found = true;
                remaining -= 1;
                break;
            }
            if (end - tail).length_squared() < EPS {
                chain.extend(pts.iter().rev().skip(1).copied());
                used[i] = true;
                found = true;
                remaining -= 1;
                break;
            }
        }
        if !found {
            return edge_pointlists.into_iter().flatten().collect();
        }
    }
    if chain.len() > 1
        && (chain[0] - *chain.last().expect("chain tidak kosong")).length_squared() < EPS
    {
        chain.pop();
    }
    chain
}

/// Hitung vektor normal satuan ke arah luar (*outward normal*) dan titik pusat (*centroid*) dari sebuah `Face`.
pub(crate) fn compute_face_normal_and_centroid(face: &Face, ray_dir: DVec3) -> Option<(DVec3, DVec3)> {
    let pts = chain_face_boundary_points(face);
    if pts.is_empty() {
        return None;
    }
    let centroid = pts.iter().fold(DVec3::ZERO, |acc, p| acc + *p) / (pts.len() as f64);

    let mut normal = DVec3::ZERO;
    for i in 0..pts.len() {
        let j = (i + 1) % pts.len();
        normal.x += (pts[i].y - pts[j].y) * (pts[i].z + pts[j].z);
        normal.y += (pts[i].z - pts[j].z) * (pts[i].x + pts[j].x);
        normal.z += (pts[i].x - pts[j].x) * (pts[i].y + pts[j].y);
    }
    let norm_len = normal.length();
    if norm_len < 1e-9 {
        return None;
    }
    let mut unit_normal = normal / norm_len;
    if unit_normal.dot(ray_dir) > 0.0 {
        unit_normal = -unit_normal;
    }
    Some((unit_normal, centroid))
}

/// Hitung arah satuan push/pull gizmo (`FaceHit::pull_dir`, CADRAW Fase 4)
pub(crate) fn compute_pull_dir(
    face: &Face,
    surface_kind: SurfaceKind,
    hit: DVec3,
    normal: DVec3,
    ray_dir: DVec3,
) -> DVec3 {
    const EPS: f64 = 1e-9;
    match surface_kind {
        SurfaceKind::Plane => normal,
        SurfaceKind::Cylinder | SurfaceKind::Cone => face
            .cylinder_or_cone_axis()
            .and_then(|(axis_location, axis_direction)| {
                let axis_direction = axis_direction.normalize();
                let proj_len = (hit - axis_location).dot(axis_direction);
                let proj_point = axis_location + axis_direction * proj_len;
                let radial = hit - proj_point;
                (radial.length() > EPS).then(|| radial.normalize())
            })
            .unwrap_or(normal),
        SurfaceKind::Sphere => {
            let mut radial = face.normal_at(hit);
            if radial.length() > EPS {
                radial = radial.normalize();
                if radial.dot(ray_dir) > 0.0 {
                    radial = -radial;
                }
                radial
            } else {
                normal
            }
        }
        SurfaceKind::Torus | SurfaceKind::Other => normal,
    }
}

/// Cast `ray` ke `shape`, kembalikan detail face (hit point, centroid, normal keluar, arah push/pull) terdekat.
pub fn pick_face_details(shape: &KernelShape, ray: PickRay) -> Option<FaceHit> {
    let _guard = lock_kernel();
    let (face, hit) = resolve_face_along_ray(shape.inner(), ray)?;
    let surface_kind = SurfaceKind::from(face.surface_kind().as_str());
    let (normal, centroid) =
        compute_face_normal_and_centroid(&face, ray.dir_vec()).unwrap_or_else(|| {
            let centroid = face.center_of_mass();
            let mut normal = face.normal_at(hit);
            if normal.dot(ray.dir_vec()) > 0.0 {
                normal = -normal;
            }
            (normal, centroid)
        });
    let pull_dir = compute_pull_dir(&face, surface_kind, hit, normal, ray.dir_vec());
    Some(FaceHit {
        hit_point: (hit.x, hit.y, hit.z),
        centroid: (centroid.x, centroid.y, centroid.z),
        normal: (normal.x, normal.y, normal.z),
        surface_kind,
        pull_dir: (pull_dir.x, pull_dir.y, pull_dir.z),
    })
}

/// Cast `ray` ke `shape`, kembalikan titik hit face TERDEKAT (kalau ada).
pub fn pick_face(shape: &KernelShape, ray: PickRay) -> Option<(f64, f64, f64)> {
    let _guard = lock_kernel();
    resolve_face_along_ray(shape.inner(), ray).map(|(_, p)| (p.x, p.y, p.z))
}
