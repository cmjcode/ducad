//! Enumerasi topologi (face & tepi) dengan indeks stabil DALAM SATU shape
//! yang sama — dasar selector semantik engine (`ducad-engine::select`).
//!
//! Agent/CLI memilih face atau tepi lewat properti (jenis, normal, posisi),
//! lalu operasi dieksekusi lewat indeks (`fillet_edges_by_index`, …). Indeks
//! hanya bermakna selama shape-nya tidak berubah: setiap operasi yang
//! menghasilkan shape baru juga menghasilkan penomoran baru.
//!
//! Kunci konsistensi: [`unique_edges`] dan [`ordered_faces`] adalah SATU-
//! SATUNYA sumber urutan. `enumerate_*` dan semua fungsi `*_by_index`
//! memanggil helper yang sama sehingga indeks `i` selalu menunjuk elemen
//! yang sama.

use std::collections::HashMap;

use glam::DVec3;
use opencascade::primitives::{Edge, EdgeType, Face, Shape};

use crate::lock_kernel;
use crate::picking::face::{chain_face_boundary_points, SurfaceKind};
use crate::shape::KernelShape;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Line,
    Circle,
    Other,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FaceInfo {
    pub index: usize,
    pub kind: SurfaceKind,
    /// `Face::center_of_mass`.
    pub centroid: [f64; 3],
    /// Normal KELUAR. Face planar: di pusat massa. Face lengkung: di titik
    /// tengah tepi terpanjang face (pusat massa silinder ada di sumbunya
    /// sehingga proyeksinya ke permukaan tidak terdefinisi).
    pub normal: [f64; 3],
    pub area: f64,
    /// Cylinder/Cone/Sphere.
    pub radius: Option<f64>,
    /// (titik, arah satuan) untuk Cylinder/Cone.
    pub axis: Option<([f64; 3], [f64; 3])>,
    /// Dari seluruh titik aproksimasi tepi face + centroid.
    pub bbox: ([f64; 3], [f64; 3]),
    /// Indeks ke [`enumerate_edges`].
    pub edge_indices: Vec<usize>,
    /// Hanya Cylinder/Cone: true = permukaan cekung (dinding lubang),
    /// false = cembung (poros).
    pub concave: Option<bool>,
    /// Poligon batas luar face di ruang 3D (`chain_face_boundary_points`).
    #[serde(skip)]
    pub boundary: Vec<[f64; 3]>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EdgeInfo {
    pub index: usize,
    pub kind: EdgeKind,
    pub start: [f64; 3],
    pub end: [f64; 3],
    /// Titik di setengah panjang busur.
    pub mid: [f64; 3],
    pub length: f64,
    /// Satuan, hanya untuk Line; komponen non-nol pertama selalu positif.
    pub dir: Option<[f64; 3]>,
    /// Hanya untuk Circle.
    pub radius: Option<f64>,
}

type EdgeKey = ((i64, i64, i64), i64);

/// Polyline aproksimasi + panjang + titik setengah panjang sebuah tepi.
struct EdgeGeom {
    points: Vec<DVec3>,
    length: f64,
    mid: DVec3,
}

fn edge_geom(edge: &Edge) -> EdgeGeom {
    let mut points: Vec<DVec3> = edge.approximation_segments().collect();
    if points.len() < 2 {
        points = vec![edge.start_point(), edge.end_point()];
    }
    let length: f64 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    let half = length * 0.5;
    let mut acc = 0.0;
    let mut mid = points[0];
    for w in points.windows(2) {
        let seg = (w[1] - w[0]).length();
        if acc + seg >= half && seg > 0.0 {
            mid = w[0] + (w[1] - w[0]) * ((half - acc) / seg);
            break;
        }
        acc += seg;
        mid = w[1];
    }
    EdgeGeom {
        points,
        length,
        mid,
    }
}

fn edge_key(g: &EdgeGeom) -> EdgeKey {
    let q = |v: f64| (v * 1e4).round() as i64;
    ((q(g.mid.x), q(g.mid.y), q(g.mid.z)), q(g.length))
}

/// Tepi unik shape dalam urutan kemunculan pertama `Shape::edges()`.
/// `TopExp_Explorer` mengunjungi tepi bersama dua kali (sekali per face
/// tetangga); duplikat dibuang lewat kunci posisi terkuantisasi
/// `(round(mid·1e4), round(length·1e4))` — pola yang sama dengan
/// `edge_dimensions`.
pub(crate) fn unique_edges(shape: &Shape) -> Vec<Edge> {
    unique_edges_with_geom(shape)
        .into_iter()
        .map(|(e, _)| e)
        .collect()
}

fn unique_edges_with_geom(shape: &Shape) -> Vec<(Edge, EdgeGeom)> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for edge in shape.edges() {
        let g = edge_geom(&edge);
        if seen.insert(edge_key(&g)) {
            out.push((edge, g));
        }
    }
    out
}

/// Face shape dalam urutan `Shape::faces()` (solid tidak berbagi face,
/// sehingga tidak perlu dedup).
pub(crate) fn ordered_faces(shape: &Shape) -> Vec<Face> {
    shape.faces().collect()
}

/// Normal keluar face di titik `p`.
///
/// Aturan final (diverifikasi tes box `topo_box_faces_outward_and_area`):
/// `n = face.normal_at(p)` APA ADANYA. `Face::normal_at` memakai
/// `BRepGProp_Face::Normal`, yang sudah membalik normal untuk face
/// berorientasi `Reversed`; membaliknya lagi (draf awal rencana) membuat
/// seluruh normal box menunjuk ke dalam — dibuktikan tes, bukan teori.
fn outward_normal_at(face: &Face, p: DVec3) -> DVec3 {
    face.normal_at(p).normalize_or_zero()
}

fn circumradius(a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let ab = b - a;
    let ac = c - a;
    let denom = 2.0 * ab.cross(ac).length();
    if denom < 1e-12 {
        return None;
    }
    Some(ab.length() * ac.length() * (b - c).length() / denom)
}

fn canonical_dir(d: DVec3) -> DVec3 {
    let d = d.normalize_or_zero();
    for c in d.to_array() {
        if c.abs() > 1e-9 {
            return if c < 0.0 { -d } else { d };
        }
    }
    d
}

fn edge_info(index: usize, edge: &Edge, g: &EdgeGeom) -> EdgeInfo {
    let kind = match edge.edge_type() {
        EdgeType::Line => EdgeKind::Line,
        EdgeType::Circle => EdgeKind::Circle,
        _ => EdgeKind::Other,
    };
    let start = edge.start_point();
    let end = edge.end_point();
    let dir = (kind == EdgeKind::Line).then(|| canonical_dir(end - start).to_array());
    let radius = if kind == EdgeKind::Circle && g.points.len() >= 3 {
        let n = g.points.len();
        circumradius(g.points[0], g.points[n / 3], g.points[(2 * n) / 3])
            .or_else(|| circumradius(g.points[0], g.points[n / 2], g.points[n - 1]))
    } else {
        None
    };
    EdgeInfo {
        index,
        kind,
        start: start.to_array(),
        end: end.to_array(),
        mid: g.mid.to_array(),
        length: g.length,
        dir,
        radius,
    }
}

/// Daftar tepi unik shape. Indeks sama dengan yang diterima
/// `fillet_edges_by_index`/`chamfer_edges_by_index`.
pub fn enumerate_edges(shape: &KernelShape) -> Vec<EdgeInfo> {
    let _guard = lock_kernel();
    unique_edges_with_geom(shape.inner())
        .iter()
        .enumerate()
        .map(|(i, (e, g))| edge_info(i, e, g))
        .collect()
}

/// Daftar face shape. Indeks sama dengan yang diterima `shell_faces_by_index`.
pub fn enumerate_faces(shape: &KernelShape) -> Vec<FaceInfo> {
    let _guard = lock_kernel();
    let inner = shape.inner();
    let edge_index: HashMap<EdgeKey, usize> = unique_edges_with_geom(inner)
        .iter()
        .enumerate()
        .map(|(i, (_, g))| (edge_key(g), i))
        .collect();

    ordered_faces(inner)
        .iter()
        .enumerate()
        .map(|(index, face)| face_info(index, face, &edge_index))
        .collect()
}

fn face_info(index: usize, face: &Face, edge_index: &HashMap<EdgeKey, usize>) -> FaceInfo {
    let kind = SurfaceKind::from(face.surface_kind().as_str());
    let centroid = face.center_of_mass();

    let mut edge_indices = Vec::new();
    let mut bb_min = centroid;
    let mut bb_max = centroid;
    let mut longest: Option<EdgeGeom> = None;
    let mut first_point: Option<DVec3> = None;
    for edge in face.edges() {
        let g = edge_geom(&edge);
        if let Some(&i) = edge_index.get(&edge_key(&g)) {
            if !edge_indices.contains(&i) {
                edge_indices.push(i);
            }
        }
        for p in &g.points {
            bb_min = bb_min.min(*p);
            bb_max = bb_max.max(*p);
        }
        first_point.get_or_insert(edge.start_point());
        if longest.as_ref().is_none_or(|l| g.length > l.length) {
            longest = Some(g);
        }
    }

    let normal = if kind == SurfaceKind::Plane {
        outward_normal_at(face, centroid)
    } else {
        outward_normal_at(face, longest.as_ref().map(|l| l.mid).unwrap_or(centroid))
    };

    let axis = match kind {
        SurfaceKind::Cylinder | SurfaceKind::Cone => face
            .cylinder_or_cone_axis()
            .map(|(p, d)| (p.to_array(), d.normalize_or_zero().to_array())),
        _ => None,
    };
    let radius = match kind {
        SurfaceKind::Cylinder | SurfaceKind::Cone => face.cylinder_or_cone_radius(),
        SurfaceKind::Sphere => face.sphere_radius(),
        _ => None,
    };
    let concave = match (axis, first_point) {
        (Some((ap, ad)), Some(p)) => {
            let (ap, ad) = (DVec3::from_array(ap), DVec3::from_array(ad));
            let n = outward_normal_at(face, p);
            let rel = p - ap;
            let radial = (rel - ad * rel.dot(ad)).normalize_or_zero();
            Some(n.dot(radial) < 0.0)
        }
        _ => None,
    };

    FaceInfo {
        index,
        kind,
        centroid: centroid.to_array(),
        normal: normal.to_array(),
        area: face.surface_area(),
        radius,
        axis,
        bbox: (bb_min.to_array(), bb_max.to_array()),
        edge_indices,
        concave,
        boundary: chain_face_boundary_points(face)
            .into_iter()
            .map(|p| p.to_array())
            .collect(),
    }
}

/// Ambil `items[i]` untuk tiap indeks (duplikat diabaikan), atau error
/// berbahasa Indonesia bila daftar kosong / indeks di luar jangkauan.
pub(crate) fn take_by_index<T>(
    items: Vec<T>,
    indices: &[usize],
    what: &str,
) -> anyhow::Result<Vec<T>> {
    if indices.is_empty() {
        anyhow::bail!("daftar indeks {what} kosong");
    }
    let n = items.len();
    if let Some(&i) = indices.iter().find(|&&i| i >= n) {
        anyhow::bail!("indeks {what} {i} di luar jangkauan (jumlah {what} {n})");
    }
    let mut slots: Vec<Option<T>> = items.into_iter().map(Some).collect();
    Ok(indices.iter().filter_map(|&i| slots[i].take()).collect())
}
