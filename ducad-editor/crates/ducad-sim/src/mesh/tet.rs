//! Mesher tetrahedral konform dari mesh permukaan.
//!
//! Alur:
//! 1. Tepi fitur + ukuran lokal → titik sampel permukaan dan interior
//!    (`tet_points`): inilah "remesh" permukaan menuju ukuran `h`.
//! 2. Delaunay 3D Bowyer–Watson dengan predikat eksak (`delaunay`).
//! 3. Tet di luar body dibuang (paritas sinar di titik berat tet). Batas
//!    himpunan tet yang tersisa adalah triangulasi Delaunay terbatas dari
//!    sampel permukaan; **konformitasnya diverifikasi, tidak diasumsikan**:
//!    batas harus manifold tertutup, semua verteksnya sampel permukaan, dan
//!    tiap sisi batas menempel pada permukaan asli. Bila tidak, mesher gagal
//!    dan pemanggil jatuh ke mesh hex voxel.
//! 4. Perbaikan kualitas (`tet_improve`) sampai rasio radius ≥ target.
//! 5. Tet4 → Tet10: node tengah tepi batas diproyeksikan ke permukaan (atau
//!    ke kurva tepi fitur), dengan pemeriksaan Jacobian.

use crate::element::tet10;
use crate::linalg::{add, cross, norm, normalize, scale, sub, V3};
use crate::mesh::delaunay::{delaunay, TetError, FACE};
use crate::mesh::spatial::{CellGrid, SurfaceIndex};
use crate::mesh::tet_improve::{build_adjacency, improve, radius_ratio, Work, NONE};
use crate::mesh::tet_points::{extract_features, generate_points, Features};
use crate::mesh::voxel::validate_surface;
use crate::setup::MeshSettings;
use crate::{CancelToken, SurfaceMesh};

/// Jumlah elemen bawaan bila ukuran tidak diberikan.
const DEFAULT_ELEMENTS: usize = 30_000;
/// Volume rata-rata satu tet relatif terhadap `h³` (kisi BCC, a = 1.12·h).
const TET_VOLUME_FACTOR: f64 = 0.117;
/// Target rasio radius saat perbaikan (gerbang: ≥ 0.1).
const QUALITY_TARGET: f64 = 0.2;
/// Rasio radius minimum yang masih diterima.
pub const MIN_RADIUS_RATIO: f64 = 0.1;

/// Sisi batas (segitiga 6-node), berorientasi keluar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TetFace {
    /// Tag face B-rep.
    pub tag: u32,
    pub elem: u32,
    /// Tiga sudut lalu tiga node tengah tepi (01, 12, 20).
    pub nodes: [u32; 6],
}

/// Mesh Tet10.
#[derive(Debug, Clone, PartialEq)]
pub struct TetMesh {
    pub nodes: Vec<V3>,
    /// Node `0..corner_count` adalah sudut tet; sisanya node tengah tepi.
    pub corner_count: usize,
    pub elems: Vec<[u32; 10]>,
    /// Sisi batas terurut menurut `(tag, elem)`.
    pub faces: Vec<TetFace>,
}

impl TetMesh {
    pub fn elem_coords(&self, e: usize) -> [V3; 10] {
        let mut x = [[0.0; 3]; 10];
        for (slot, &n) in x.iter_mut().zip(&self.elems[e]) {
            *slot = self.nodes[n as usize];
        }
        x
    }

    /// Volume total (kuadratur pada elemen lengkung), mm³.
    pub fn volume(&self) -> f64 {
        (0..self.elems.len())
            .map(|e| tet10::volume(&self.elem_coords(e)))
            .sum()
    }

    /// Sisi batas bertag `tag`.
    pub fn faces_of(&self, tag: u32) -> &[TetFace] {
        let start = self.faces.partition_point(|f| f.tag < tag);
        let end = self.faces.partition_point(|f| f.tag <= tag);
        &self.faces[start..end]
    }

    /// Node (terurut, unik) pada sisi batas bertag `tag`.
    pub fn face_nodes(&self, tag: u32) -> Vec<u32> {
        let mut nodes: Vec<u32> = self.faces_of(tag).iter().flat_map(|f| f.nodes).collect();
        nodes.sort_unstable();
        nodes.dedup();
        nodes
    }

    /// Tag face (terurut, unik) yang punya sisi batas.
    pub fn tagged_faces(&self) -> Vec<u32> {
        let mut tags: Vec<u32> = self.faces.iter().map(|f| f.tag).collect();
        tags.dedup();
        tags
    }

    /// Rasio radius (sudut-sudut saja) elemen `e`.
    pub fn radius_ratio(&self, e: usize) -> f64 {
        let c = self.elems[e];
        radius_ratio(
            self.nodes[c[0] as usize],
            self.nodes[c[1] as usize],
            self.nodes[c[2] as usize],
            self.nodes[c[3] as usize],
        )
    }

    /// Jumlah elemen dengan Jacobian tidak positif di titik Gauss atau node.
    pub fn inverted_count(&self) -> usize {
        (0..self.elems.len())
            .filter(|&e| {
                let det = tet10::min_det(&self.elem_coords(e));
                det.is_nan() || det <= 0.0
            })
            .count()
    }
}

/// Mesh tet + catatan pembuatannya.
#[derive(Debug, Clone, PartialEq)]
pub struct TetModel {
    pub mesh: TetMesh,
    /// Ukuran tepi target dasar, mm.
    pub size_mm: f64,
    /// Volume acuan (eksak dari B-rep, atau volume mesh permukaan), mm³.
    pub target_volume_mm3: f64,
    pub min_radius_ratio: f64,
    /// Tag face (terurut, unik) yang ada di mesh permukaan masukan.
    pub surface_tags: Vec<u32>,
    pub warnings: Vec<String>,
}

fn check_cancel(cancel: &CancelToken) -> Result<(), TetError> {
    if cancel.is_cancelled() {
        Err(TetError::Cancelled)
    } else {
        Ok(())
    }
}

/// Sisi batas mesh kerja: `(tet, indeks lokal verteks seberang)`.
fn boundary_faces(work: &Work, adj: &[[u32; 4]]) -> Vec<(u32, u8)> {
    let mut out = Vec::new();
    for (t, a) in adj.iter().enumerate() {
        for i in 0..4 {
            if a[i] == NONE {
                out.push((t as u32, i as u8));
            }
            let _ = work;
        }
    }
    out
}

/// Memeriksa bahwa batas mesh kerja benar-benar mengikuti permukaan.
fn validate_boundary(work: &Work, index: &SurfaceIndex) -> Result<(), TetError> {
    if work.tets.is_empty() {
        return Err(TetError::failed("no tetrahedra inside the body"));
    }
    let adj = build_adjacency(&work.tets)?;
    let faces = boundary_faces(work, &adj);
    let mut edges: Vec<(u32, u32)> = Vec::with_capacity(3 * faces.len());
    for &(t, i) in &faces {
        let v = work.tets[t as usize];
        let f = FACE[i as usize];
        let tri = [v[f[0]], v[f[1]], v[f[2]]];
        if tri.iter().any(|&n| n as usize >= work.n_surface) {
            return Err(TetError::failed(
                "boundary recovery failed: an interior point lies on the mesh boundary",
            ));
        }
        let p = [
            work.pts[tri[0] as usize],
            work.pts[tri[1] as usize],
            work.pts[tri[2] as usize],
        ];
        let centroid = scale(add(p[0], add(p[1], p[2])), 1.0 / 3.0);
        let longest = norm(sub(p[1], p[0]))
            .max(norm(sub(p[2], p[1])))
            .max(norm(sub(p[0], p[2])));
        let near = index
            .closest(centroid, None, None)
            .is_some_and(|c| c.dist <= 0.25 * longest);
        if !near {
            return Err(TetError::failed(
                "boundary recovery failed: a boundary facet does not lie on the surface",
            ));
        }
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            edges.push((a.min(b), a.max(b)));
        }
    }
    edges.sort_unstable();
    for pair in edges.chunks(2) {
        if pair.len() != 2 || pair[0] != pair[1] {
            return Err(TetError::failed(
                "boundary recovery failed: the mesh boundary is not a closed manifold",
            ));
        }
    }
    for chunk in edges.chunks(4) {
        if chunk.len() == 4 && chunk[0] == chunk[3] {
            return Err(TetError::failed(
                "boundary recovery failed: the mesh boundary pinches along an edge",
            ));
        }
    }
    Ok(())
}

/// Titik terdekat pada segmen-segmen tepi fitur dalam radius `radius`.
struct SegmentIndex<'a> {
    features: &'a Features,
    grid: CellGrid,
}

impl<'a> SegmentIndex<'a> {
    fn new(features: &'a Features, min: V3, max: V3, cell: f64) -> SegmentIndex<'a> {
        let mut grid = CellGrid::new(min, max, cell, 250_000);
        let mut pairs = Vec::new();
        for (s, seg) in features.segments.iter().enumerate() {
            let (a, b) = (
                features.wpos[seg[0] as usize],
                features.wpos[seg[1] as usize],
            );
            let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
            let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
            grid.push_box(lo, hi, s as u32, &mut pairs);
        }
        grid.fill(pairs);
        SegmentIndex { features, grid }
    }

    fn closest(&self, p: V3, radius: f64) -> Option<(f64, V3)> {
        let lo = self
            .grid
            .cell_of([p[0] - radius, p[1] - radius, p[2] - radius]);
        let hi = self
            .grid
            .cell_of([p[0] + radius, p[1] + radius, p[2] + radius]);
        let mut best: Option<(f64, V3)> = None;
        for k in lo[2]..=hi[2] {
            for j in lo[1]..=hi[1] {
                for i in lo[0]..=hi[0] {
                    for &s in self.grid.items([i, j, k]) {
                        let seg = self.features.segments[s as usize];
                        let (a, b) = (
                            self.features.wpos[seg[0] as usize],
                            self.features.wpos[seg[1] as usize],
                        );
                        let ab = sub(b, a);
                        let len2 = crate::linalg::dot(ab, ab);
                        let t = if len2 > 0.0 {
                            (crate::linalg::dot(sub(p, a), ab) / len2).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let q = add(a, scale(ab, t));
                        let d = norm(sub(p, q));
                        if d <= radius && best.is_none_or(|b| d < b.0) {
                            best = Some((d, q));
                        }
                    }
                }
            }
        }
        best
    }
}

/// Mengubah mesh kerja linier menjadi Tet10 dengan node tengah tepi batas
/// diproyeksikan ke permukaan.
fn build_tet10(
    work: &Work,
    index: &SurfaceIndex,
    features: &Features,
    min: V3,
    max: V3,
    h: f64,
) -> Result<TetMesh, TetError> {
    // Buang verteks yang tidak terpakai.
    let mut remap = vec![NONE; work.pts.len()];
    let mut nodes: Vec<V3> = Vec::new();
    for t in &work.tets {
        for &v in t {
            remap[v as usize] = 0;
        }
    }
    for (v, slot) in remap.iter_mut().enumerate() {
        if *slot == 0 {
            *slot = nodes.len() as u32;
            nodes.push(work.pts[v]);
        }
    }
    let tets: Vec<[u32; 4]> = work
        .tets
        .iter()
        .map(|t| {
            [
                remap[t[0] as usize],
                remap[t[1] as usize],
                remap[t[2] as usize],
                remap[t[3] as usize],
            ]
        })
        .collect();
    let corner_count = nodes.len();
    // Tepi unik → node tengah.
    let mut edges: Vec<(u32, u32)> = Vec::with_capacity(6 * tets.len());
    for t in &tets {
        for e in tet10::EDGES {
            let (a, b) = (t[e[0]], t[e[1]]);
            edges.push((a.min(b), a.max(b)));
        }
    }
    edges.sort_unstable();
    edges.dedup();
    let mid_of = |a: u32, b: u32| -> u32 {
        let key = (a.min(b), a.max(b));
        (corner_count + edges.partition_point(|e| *e < key)) as u32
    };
    for &(a, b) in &edges {
        nodes.push(scale(add(nodes[a as usize], nodes[b as usize]), 0.5));
    }
    let elems: Vec<[u32; 10]> = tets
        .iter()
        .map(|t| {
            let mut c = [0u32; 10];
            c[..4].copy_from_slice(t);
            for (k, e) in tet10::EDGES.iter().enumerate() {
                c[4 + k] = mid_of(t[e[0]], t[e[1]]);
            }
            c
        })
        .collect();
    // Sisi batas + tag.
    let adj = build_adjacency(&tets)?;
    let mut faces: Vec<TetFace> = Vec::new();
    let mut edge_tags: Vec<(u32, u32)> = Vec::new(); // (node tengah, tag)
    for (t, a) in adj.iter().enumerate() {
        for i in 0..4 {
            if a[i] != NONE {
                continue;
            }
            let f = FACE[i];
            // FACE berorientasi ke dalam; tukar dua verteks agar keluar.
            let tri = [tets[t][f[0]], tets[t][f[2]], tets[t][f[1]]];
            let p = [
                nodes[tri[0] as usize],
                nodes[tri[1] as usize],
                nodes[tri[2] as usize],
            ];
            let centroid = scale(add(p[0], add(p[1], p[2])), 1.0 / 3.0);
            let normal = normalize(cross(sub(p[1], p[0]), sub(p[2], p[0]))).unwrap_or([0.0; 3]);
            let longest = norm(sub(p[1], p[0]))
                .max(norm(sub(p[2], p[1])))
                .max(norm(sub(p[0], p[2])));
            let tag = index
                .closest(centroid, None, Some((normal, 0.1 * longest)))
                .map(|c| index.surface.tri_face[c.tri as usize])
                .ok_or_else(|| TetError::failed("could not tag a boundary facet"))?;
            let mids = [
                mid_of(tri[0], tri[1]),
                mid_of(tri[1], tri[2]),
                mid_of(tri[2], tri[0]),
            ];
            for m in mids {
                edge_tags.push((m, tag));
            }
            faces.push(TetFace {
                tag,
                elem: t as u32,
                nodes: [tri[0], tri[1], tri[2], mids[0], mids[1], mids[2]],
            });
        }
    }
    faces.sort_by_key(|f| (f.tag, f.elem, f.nodes));
    // Proyeksi node tengah tepi batas.
    edge_tags.sort_unstable();
    edge_tags.dedup();
    let segments = SegmentIndex::new(features, min, max, h);
    let straight: Vec<V3> = nodes.clone();
    let mut moved = vec![false; nodes.len()];
    let mut s = 0;
    while s < edge_tags.len() {
        let mut e = s;
        while e < edge_tags.len() && edge_tags[e].0 == edge_tags[s].0 {
            e += 1;
        }
        let m = edge_tags[s].0 as usize;
        let (a, b) = edges[m - corner_count];
        let length = norm(sub(nodes[a as usize], nodes[b as usize]));
        let limit = 0.3 * length;
        let target = if e - s == 1 {
            index
                .closest(straight[m], Some(edge_tags[s].1), None)
                .filter(|c| c.dist <= limit)
                .map(|c| c.point)
        } else {
            segments.closest(straight[m], limit).map(|c| c.1)
        };
        if let Some(q) = target {
            if norm(sub(q, straight[m])) > 1e-12 * length {
                nodes[m] = q;
                moved[m] = true;
            }
        }
        s = e;
    }
    // Jacobian: tarik kembali node yang membuat elemen terlalu terdistorsi.
    let mut mesh = TetMesh {
        nodes,
        corner_count,
        elems,
        faces,
    };
    for round in 0..5 {
        let mut any = false;
        for e in 0..mesh.elems.len() {
            let conn = mesh.elems[e];
            if !conn[4..].iter().any(|&m| moved[m as usize]) {
                continue;
            }
            let x = mesh.elem_coords(e);
            let mut flat = x;
            for k in 4..10 {
                flat[k] = straight[conn[k] as usize];
            }
            let reference = tet10::phys_grad(&flat, [0.25; 4]).1;
            if tet10::min_det(&x) > 0.3 * reference {
                continue;
            }
            any = true;
            for &m in &conn[4..] {
                let m = m as usize;
                if moved[m] {
                    mesh.nodes[m] = if round < 4 {
                        scale(add(mesh.nodes[m], straight[m]), 0.5)
                    } else {
                        straight[m]
                    };
                }
            }
        }
        if !any {
            break;
        }
    }
    Ok(mesh)
}

/// Membuat mesh Tet10 konform. `fine_faces` = tag face yang dibebani atau
/// dikunci (diperhalus). Mengembalikan galat bila batas tidak bisa
/// dipulihkan; pemanggil lalu memakai mesh hex voxel.
pub fn tetrahedralize(
    surface: &SurfaceMesh,
    settings: &MeshSettings,
    fine_faces: &[u32],
    exact_volume_mm3: Option<f64>,
    cancel: &CancelToken,
) -> Result<TetModel, TetError> {
    let (min, extent, surface_volume) =
        validate_surface(surface).map_err(|e| TetError::failed(e.to_string()))?;
    let max = add(min, extent);
    let diag = norm(extent);
    let target_volume = match exact_volume_mm3 {
        Some(v) if v.is_finite() && v > 0.0 => v,
        Some(_) => return Err(TetError::failed("exact volume must be a positive number")),
        None => surface_volume,
    };
    let h = match settings.cell_mm {
        Some(h) if h.is_finite() && h > 0.0 => h,
        Some(_) => return Err(TetError::failed("mesh.cell_mm must be a positive number")),
        None => {
            let n = settings.target_elems.unwrap_or(DEFAULT_ELEMENTS).max(1);
            (surface_volume / (TET_VOLUME_FACTOR * n as f64)).cbrt()
        }
    };
    if h < diag / 2000.0 {
        return Err(TetError::failed("mesh size is too small for this body"));
    }
    let features = extract_features(surface, diag, h, fine_faces);
    let index = SurfaceIndex::new(surface, min, max, h);
    let cloud = generate_points(surface, &index, &features, min, max, h, cancel)?;
    check_cancel(cancel)?;
    let triangulation = delaunay(&cloud.pts, cancel)?;
    check_cancel(cancel)?;
    let mut kept = Vec::with_capacity(triangulation.tets.len() / 2);
    for t in &triangulation.tets {
        let c = scale(
            add(
                add(cloud.pts[t[0] as usize], cloud.pts[t[1] as usize]),
                add(cloud.pts[t[2] as usize], cloud.pts[t[3] as usize]),
            ),
            0.25,
        );
        if index.inside(c) {
            kept.push(*t);
        }
    }
    let mut work = Work {
        pts: cloud.pts,
        tets: kept,
        n_surface: cloud.n_surface,
        tags: cloud.tags,
    };
    validate_boundary(&work, &index)?;
    check_cancel(cancel)?;
    improve(&mut work, &index, QUALITY_TARGET, 40, cancel)?;
    validate_boundary(&work, &index)?;
    let mesh = build_tet10(&work, &index, &features, min, max, h)?;
    check_cancel(cancel)?;

    let mut warnings = Vec::new();
    if mesh.inverted_count() > 0 {
        return Err(TetError::failed("the tet mesh contains inverted elements"));
    }
    let min_ratio = (0..mesh.elems.len())
        .map(|e| mesh.radius_ratio(e))
        .fold(f64::INFINITY, f64::min);
    if min_ratio < 0.02 {
        return Err(TetError::failed(format!(
            "tet quality too low (minimum radius ratio {min_ratio:.4})"
        )));
    }
    if min_ratio < MIN_RADIUS_RATIO {
        warnings.push(format!(
            "minimum tet radius ratio is {min_ratio:.3} (below {MIN_RADIUS_RATIO}); results near the worst elements are less accurate"
        ));
    }
    let volume = mesh.volume();
    if (volume - surface_volume).abs() > 0.01 * surface_volume {
        return Err(TetError::failed(format!(
            "tet mesh volume {volume:.6e} differs from the surface volume {surface_volume:.6e} by more than 1 %"
        )));
    }
    let mut surface_tags = surface.tri_face.clone();
    surface_tags.sort_unstable();
    surface_tags.dedup();
    Ok(TetModel {
        mesh,
        surface_tags,
        size_mm: h,
        target_volume_mm3: target_volume,
        min_radius_ratio: min_ratio,
        warnings,
    })
}
