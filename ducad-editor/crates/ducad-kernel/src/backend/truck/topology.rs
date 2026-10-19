//! Topologi LOGIS sebuah body truck: face dan edge sebagaimana DUCAD (dan
//! OCCT) melihatnya.
//!
//! SolidCraft memecah permukaan periodik: sisi silinder penuh jadi dua face,
//! lingkaran jadi dua busur. OCCT menyimpannya sebagai satu face (dengan satu
//! edge seam) dan satu edge, dan setiap boolean DUCAD di OCCT diakhiri
//! `clean()` (UnifySameDomain) yang menggabung face/edge sebidang. Supaya
//! indeks `enumerate_faces/edges`, selector engine, dan hitungan golden
//! bermakna sama di kedua backend, lapisan ini:
//!
//! 1. menggabung face bertetangga yang terletak pada permukaan analitik yang
//!    SAMA (bidang, silinder, kerucut, bola, torus);
//! 2. membuang edge di antara potongan face yang digabung — kecuali satu
//!    "seam" bila potongan-potongannya membentuk cincin penuh (seperti OCCT);
//! 3. menyambung rantai edge yang merupakan potongan kurva yang sama
//!    (busur satu lingkaran, garis segaris) di vertex yang hanya
//!    menghubungkan keduanya.
//!
//! Dihitung sekali per shape dan di-cache di `KernelShape`.

use std::collections::HashMap;

use glam::DVec3;
use solidcraft_kernel::{ducad_ext, Body, FaceSurface};

use super::bridge::to_dv;
use crate::mesh::KernelMesh;
use crate::picking::face::SurfaceKind;
use crate::topo::EdgeKind;

/// Toleransi chord tesselasi tampilan — sama dengan `Shape::mesh()` OCCT
/// (`clamp(diag·3e-4, 0.005, 0.05)` mm, sudut 0,1 rad).
pub(crate) fn display_tolerance(size: f64) -> f64 {
    (size * 3e-4).clamp(0.005, 0.05)
}
const DISPLAY_ANGLE_RAD: f64 = 0.10;

/// Face logis: satu atau beberapa face SolidCraft pada permukaan yang sama.
pub(crate) struct LFace {
    /// Indeks face SolidCraft (urutan `face_iter`). Dipakai op by-index F3.
    #[allow(dead_code)]
    pub members: Vec<usize>,
    pub kind: SurfaceKind,
    pub surface: Option<FaceSurface>,
    pub area: f64,
    pub centroid: DVec3,
    /// Edge logis yang membatasi face ini, urut kemunculan di wire.
    pub edges: Vec<usize>,
}

/// Edge logis: rantai satu atau beberapa edge SolidCraft.
pub(crate) struct LEdge {
    /// Indeks edge SolidCraft (urutan `Body::edges`), urut sepanjang rantai.
    /// Dipakai op by-index F3 (fillet/chamfer memilih edge lewat titiknya).
    #[allow(dead_code)]
    pub members: Vec<usize>,
    /// Polyline dari awal ke akhir rantai.
    pub points: Vec<DVec3>,
    pub kind: EdgeKind,
    pub radius: Option<f64>,
    /// Face logis di kiri-kanan edge.
    pub faces: Vec<usize>,
}

impl LEdge {
    pub fn start(&self) -> DVec3 {
        self.points.first().copied().unwrap_or(DVec3::ZERO)
    }
    pub fn end(&self) -> DVec3 {
        self.points.last().copied().unwrap_or(DVec3::ZERO)
    }
    pub fn length(&self) -> f64 {
        self.points.windows(2).map(|w| (w[1] - w[0]).length()).sum()
    }
    /// Titik di setengah panjang polyline.
    pub fn mid(&self) -> DVec3 {
        let half = self.length() * 0.5;
        let mut acc = 0.0;
        for w in self.points.windows(2) {
            let seg = (w[1] - w[0]).length();
            if acc + seg >= half && seg > 0.0 {
                return w[0] + (w[1] - w[0]) * ((half - acc) / seg);
            }
            acc += seg;
        }
        self.end()
    }
}

/// Topologi logis + mesh tampilan yang segitiganya diurutkan per face logis
/// (`KernelMesh::face_ranges[i]` = face logis `i`).
pub(crate) struct Topology {
    pub mesh: KernelMesh,
    pub faces: Vec<LFace>,
    pub edges: Vec<LEdge>,
    pub vertices: Vec<DVec3>,
}

struct UnionFind(Vec<usize>);

impl UnionFind {
    fn new(n: usize) -> Self {
        UnionFind((0..n).collect())
    }
    fn find(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            // Akar = indeks terkecil, supaya urutan grup stabil.
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.0[hi] = lo;
        }
    }
}

fn same_surface(a: &FaceSurface, b: &FaceSurface, lin: f64) -> bool {
    const PAR: f64 = 1.0 - 1e-9;
    match (*a, *b) {
        (
            FaceSurface::Plane {
                normal: n1,
                point: p1,
            },
            FaceSurface::Plane {
                normal: n2,
                point: p2,
            },
        ) => n1.dot(*n2) > PAR && (p2 - p1).dot(*n1).abs() < lin,
        (
            FaceSurface::Cylinder {
                origin: o1,
                axis: a1,
                radius: r1,
            },
            FaceSurface::Cylinder {
                origin: o2,
                axis: a2,
                radius: r2,
            },
        ) => {
            let off = o2 - o1;
            a1.dot(*a2).abs() > PAR
                && (r1 - r2).abs() < lin
                && (off - *a1 * off.dot(*a1)).len() < lin
        }
        (
            FaceSurface::Cone {
                apex: p1,
                axis: a1,
                half_angle: h1,
            },
            FaceSurface::Cone {
                apex: p2,
                axis: a2,
                half_angle: h2,
            },
        ) => p1.dist(p2) < lin && a1.dot(*a2) > PAR && (h1 - h2).abs() < 1e-9,
        (
            FaceSurface::Sphere {
                center: c1,
                radius: r1,
            },
            FaceSurface::Sphere {
                center: c2,
                radius: r2,
            },
        ) => c1.dist(c2) < lin && (r1 - r2).abs() < lin,
        (
            FaceSurface::Torus {
                center: c1,
                axis: a1,
                major: m1,
                minor: n1,
            },
            FaceSurface::Torus {
                center: c2,
                axis: a2,
                major: m2,
                minor: n2,
            },
        ) => {
            c1.dist(c2) < lin
                && a1.dot(*a2).abs() > PAR
                && (m1 - m2).abs() < lin
                && (n1 - n2).abs() < lin
        }
        _ => false,
    }
}

fn surface_kind(s: Option<&FaceSurface>) -> Option<SurfaceKind> {
    Some(match s? {
        FaceSurface::Plane { .. } => SurfaceKind::Plane,
        FaceSurface::Cylinder { .. } => SurfaceKind::Cylinder,
        FaceSurface::Cone { .. } => SurfaceKind::Cone,
        FaceSurface::Sphere { .. } => SurfaceKind::Sphere,
        FaceSurface::Torus { .. } => SurfaceKind::Torus,
    })
}

fn periodic(s: Option<&FaceSurface>) -> bool {
    !matches!(s, None | Some(FaceSurface::Plane { .. }))
}

/// Lingkaran sebuah edge (pusat, sumbu satuan, radius).
type Circle = (DVec3, DVec3, f64);

fn same_circle(a: &Circle, b: &Circle, lin: f64) -> bool {
    (a.0 - b.0).length() < lin && a.1.dot(b.1).abs() > 1.0 - 1e-9 && (a.2 - b.2).abs() < lin
}

/// Arah satuan polyline bila semua titiknya segaris (dalam `lin`).
fn straight_dir(points: &[DVec3], lin: f64) -> Option<DVec3> {
    let (a, b) = (*points.first()?, *points.last()?);
    let d = (b - a).normalize_or_zero();
    if d == DVec3::ZERO {
        return None;
    }
    points
        .iter()
        .all(|p| {
            let r = *p - a;
            (r - d * r.dot(d)).length() < lin
        })
        .then_some(d)
}

type Key = (i64, i64, i64);

impl Topology {
    pub fn empty() -> Topology {
        Topology {
            mesh: KernelMesh::default(),
            faces: Vec::new(),
            edges: Vec::new(),
            vertices: Vec::new(),
        }
    }

    /// Bangun topologi logis `body`. Tidak pernah gagal: bila kernel menolak
    /// sebagian kueri, hasilnya merosot (mis. hanya mesh tanpa edge) dan
    /// alasannya dicatat di log.
    pub fn build(body: &Body) -> Topology {
        let size = body.size();
        let tol = display_tolerance(size);
        let lin = (size * 1e-6).max(1e-7);
        let mesh = solidcraft_kernel::guard("display mesh", || {
            body.display_mesh_angular(tol, DISPLAY_ANGLE_RAD)
        })
        .or_else(|_| solidcraft_kernel::guard("tessellate", || body.tessellate(tol)));
        let mesh = match mesh {
            Ok(m) => m,
            Err(e) => {
                log::warn!("topologi truck: tesselasi gagal ({e}); body tampil kosong");
                return Topology::empty();
            }
        };
        let nf = body.face_count();

        // --- Adjacency face ↔ edge dari wire B-rep. ---
        let wires = ducad_ext::face_wires(body).unwrap_or_else(|e| {
            log::warn!("topologi truck: wire face tidak terbaca ({e})");
            Vec::new()
        });
        let ends = ducad_ext::edge_ends(body).unwrap_or_default();
        let ne = ends.len().min(mesh.edges.len());
        let mut edge_faces: Vec<Vec<usize>> = vec![Vec::new(); ne];
        for (f, face_wires) in wires.iter().enumerate() {
            for u in face_wires.iter().flatten() {
                if let Some(list) = edge_faces.get_mut(u.edge) {
                    if !list.contains(&f) {
                        list.push(f);
                    }
                }
            }
        }

        // --- 1. Gabung face pada permukaan analitik yang sama. ---
        let surfaces: Vec<Option<FaceSurface>> = (0..nf)
            .map(|i| {
                solidcraft_kernel::guard("face surface", || {
                    Ok(solidcraft_kernel::face_surface(body, i))
                })
                .ok()
                .flatten()
            })
            .collect();
        let mut fuf = UnionFind::new(nf);
        for faces in &edge_faces {
            if let [a, b] = faces[..] {
                if let (Some(Some(sa)), Some(Some(sb))) = (surfaces.get(a), surfaces.get(b)) {
                    if same_surface(sa, sb, lin) {
                        fuf.union(a, b);
                    }
                }
            }
        }
        let mut group_of_root: HashMap<usize, usize> = HashMap::new();
        let mut group_of = vec![0usize; nf];
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for (f, slot) in group_of.iter_mut().enumerate() {
            let root = fuf.find(f);
            let g = *group_of_root.entry(root).or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            *slot = g;
            groups[g].push(f);
        }

        // --- 2. Edge di antara potongan face sama: buang, sisakan seam cincin. ---
        let lfaces_of = |e: usize| -> Vec<usize> {
            let mut v: Vec<usize> = edge_faces[e]
                .iter()
                .filter_map(|f| group_of.get(*f).copied())
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let mut keep = vec![true; ne];
        let mut internal: Vec<Vec<usize>> = vec![Vec::new(); groups.len()];
        for (e, kept) in keep.iter_mut().enumerate() {
            let lf = lfaces_of(e);
            if lf.len() == 1 && edge_faces[e].len() >= 2 {
                *kept = false;
                internal[lf[0]].push(e);
            }
        }
        for (g, inner) in internal.iter().enumerate() {
            let members = &groups[g];
            let ring = members.len() >= 2 && inner.len() >= members.len();
            if ring && periodic(members.first().and_then(|&f| surfaces[f].as_ref())) {
                if let Some(&e) = inner.first() {
                    keep[e] = true;
                }
            }
        }

        // --- 3. Polyline tiap edge, arah depan → belakang. ---
        let polylines: Vec<Vec<DVec3>> = (0..ne)
            .map(|e| {
                let mut pts: Vec<DVec3> = mesh.edges[e].iter().map(|p| to_dv(*p)).collect();
                let (front, back) = (to_dv(ends[e].0), to_dv(ends[e].1));
                if let (Some(first), Some(last)) = (pts.first().copied(), pts.last().copied()) {
                    if (front - back).length() > lin
                        && (first - front).length() > (last - front).length()
                    {
                        pts.reverse();
                    }
                }
                if pts.len() < 2 {
                    pts = vec![front, back];
                }
                pts
            })
            .collect();
        let circles: Vec<Option<Circle>> = (0..ne)
            .map(|e| {
                solidcraft_kernel::guard("edge circle", || {
                    Ok(solidcraft_kernel::edge_circle(body, e))
                })
                .ok()
                .flatten()
                .map(|c| (to_dv(c.center), to_dv(*c.axis), c.radius))
            })
            .collect();
        let straight: Vec<Option<DVec3>> = polylines.iter().map(|p| straight_dir(p, lin)).collect();

        // --- 4. Sambung potongan kurva yang sama di vertex berderajat dua. ---
        let q = move |p: DVec3| -> Key {
            (
                (p.x / lin).round() as i64,
                (p.y / lin).round() as i64,
                (p.z / lin).round() as i64,
            )
        };
        let closed_edge = |e: usize| (to_dv(ends[e].0) - to_dv(ends[e].1)).length() < lin;
        let mut incident: HashMap<Key, Vec<usize>> = HashMap::new();
        for e in (0..ne).filter(|&e| keep[e] && !closed_edge(e)) {
            incident.entry(q(to_dv(ends[e].0))).or_default().push(e);
            incident.entry(q(to_dv(ends[e].1))).or_default().push(e);
        }
        let mut euf = UnionFind::new(ne);
        for list in incident.values() {
            if let [a, b] = list[..] {
                if a == b || lfaces_of(a) != lfaces_of(b) {
                    continue;
                }
                let same_curve = match (&circles[a], &circles[b]) {
                    (Some(ca), Some(cb)) => same_circle(ca, cb, lin),
                    _ => matches!(
                        (straight[a], straight[b]),
                        (Some(da), Some(db)) if da.dot(db).abs() > 1.0 - 1e-9
                    ),
                };
                if same_curve {
                    euf.union(a, b);
                }
            }
        }
        let mut chains: Vec<Vec<usize>> = Vec::new();
        let mut chain_of: HashMap<usize, usize> = HashMap::new();
        for e in (0..ne).filter(|&e| keep[e]) {
            let root = euf.find(e);
            let c = *chain_of.entry(root).or_insert_with(|| {
                chains.push(Vec::new());
                chains.len() - 1
            });
            chains[c].push(e);
        }

        let mut logical_of_edge: Vec<Option<usize>> = vec![None; ne];
        let mut edges: Vec<LEdge> = Vec::with_capacity(chains.len());
        // Vertex yang disentuh rantai lain: rantai TERTUTUP (lingkaran dari
        // dua busur) dimulai di sana, seperti vertex seam satu-satunya di OCCT.
        let mut touch: HashMap<Key, Vec<usize>> = HashMap::new();
        for (c, chain) in chains.iter().enumerate() {
            for &e in chain {
                let p = &polylines[e];
                for k in [q(p[0]), q(p[p.len() - 1])] {
                    let v = touch.entry(k).or_default();
                    if !v.contains(&c) {
                        v.push(c);
                    }
                }
            }
        }
        for (ci, chain) in chains.iter().enumerate() {
            let shared = |k: Key| touch.get(&k).is_some_and(|v| v.iter().any(|&c| c != ci));
            let (members, mut points) = order_chain(chain, &polylines, &q, &shared);
            let chain_circles = members
                .iter()
                .map(|&e| circles[e])
                .collect::<Option<Vec<Circle>>>()
                .filter(|cs| cs.windows(2).all(|w| same_circle(&w[0], &w[1], lin)));
            let (kind, radius) = match chain_circles {
                Some(cs) if !cs.is_empty() => (EdgeKind::Circle, Some(cs[0].2)),
                _ if straight_dir(&points, lin).is_some() => (EdgeKind::Line, None),
                _ => (EdgeKind::Other, None),
            };
            // Garis cukup dua titik (aproksimasi garis OCCT juga dua titik);
            // mesh tampilan SolidCraft membagi edge lurus jadi beberapa node.
            if kind == EdgeKind::Line && points.len() > 2 {
                points = vec![points[0], points[points.len() - 1]];
            }
            let li = edges.len();
            for &e in &members {
                logical_of_edge[e] = Some(li);
            }
            edges.push(LEdge {
                faces: members.first().map(|&e| lfaces_of(e)).unwrap_or_default(),
                members,
                points,
                kind,
                radius,
            });
        }

        // --- 5. Mesh diurutkan per face logis; ukuran face dari segitiga. ---
        // Body mesh (STL) tidak punya face B-rep: semua segitiga satu face.
        if groups.is_empty() && !mesh.triangles.is_empty() {
            groups.push(Vec::new());
        }
        let ng = groups.len();
        let mut tris_of: Vec<Vec<usize>> = vec![Vec::new(); ng];
        for t in 0..mesh.triangles.len() {
            let g = if nf == 0 {
                Some(0)
            } else {
                mesh.tri_face
                    .get(t)
                    .and_then(|f| group_of.get(*f as usize))
                    .copied()
            };
            if let Some(list) = g.and_then(|g| tris_of.get_mut(g)) {
                list.push(t);
            }
        }
        let mut km = KernelMesh {
            positions: mesh
                .positions
                .iter()
                .map(|p| [p.x as f32, p.y as f32, p.z as f32])
                .collect(),
            normals: mesh
                .normals
                .iter()
                .map(|n| [n.x as f32, n.y as f32, n.z as f32])
                .collect(),
            indices: Vec::with_capacity(mesh.triangles.len() * 3),
            face_ranges: Vec::with_capacity(ng),
        };
        if km.normals.len() != km.positions.len() {
            km.normals = vec![[0.0, 0.0, 1.0]; km.positions.len()];
        }
        let mut faces: Vec<LFace> = Vec::with_capacity(ng);
        for (g, members) in groups.iter().enumerate() {
            let start = km.indices.len() as u32;
            let (mut area, mut moment) = (0.0f64, DVec3::ZERO);
            let mut first_normal: Option<DVec3> = None;
            let mut planar = true;
            for &t in &tris_of[g] {
                let tri = mesh.triangles[t];
                km.indices.extend_from_slice(&tri);
                if let Some([a, b, c]) = mesh.tri(&tri) {
                    let (a, b, c) = (to_dv(a), to_dv(b), to_dv(c));
                    let cr = (b - a).cross(c - a);
                    let ar = cr.length() * 0.5;
                    area += ar;
                    moment += (a + b + c) * (ar / 3.0);
                    let n = cr.normalize_or_zero();
                    if n != DVec3::ZERO {
                        match first_normal {
                            None => first_normal = Some(n),
                            Some(p) if p.dot(n) < 1.0 - 1e-6 => planar = false,
                            _ => {}
                        }
                    }
                }
            }
            km.face_ranges.push(start..km.indices.len() as u32);
            let surface = members.first().and_then(|&f| surfaces[f]);
            let kind =
                surface_kind(surface.as_ref()).unwrap_or(if planar && first_normal.is_some() {
                    SurfaceKind::Plane
                } else {
                    SurfaceKind::Other
                });
            // Edge face ini, urut kemunculan di wire anggotanya.
            let mut fedges = Vec::new();
            for &f in members {
                for u in wires.get(f).into_iter().flatten().flatten() {
                    if let Some(Some(l)) = logical_of_edge.get(u.edge) {
                        if !fedges.contains(l) {
                            fedges.push(*l);
                        }
                    }
                }
            }
            faces.push(LFace {
                members: members.clone(),
                kind,
                surface,
                area,
                centroid: if area > 0.0 {
                    moment / area
                } else {
                    DVec3::ZERO
                },
                edges: fedges,
            });
        }

        // --- 6. Vertex logis: ujung edge logis (dedup 1e-6 seperti OCCT). ---
        let mut vertices: Vec<DVec3> = Vec::new();
        for e in &edges {
            for p in [e.start(), e.end()] {
                if !vertices.iter().any(|v| (*v - p).length() < 1e-6) {
                    vertices.push(p);
                }
            }
        }

        Topology {
            mesh: km,
            faces,
            edges,
            vertices,
        }
    }

    /// Segitiga face logis `f`.
    pub fn face_triangles(&self, f: usize) -> impl Iterator<Item = [DVec3; 3]> + '_ {
        let range = self.mesh.face_ranges.get(f).cloned().unwrap_or(0..0);
        let idx = self
            .mesh
            .indices
            .get(range.start as usize..range.end as usize)
            .unwrap_or(&[]);
        idx.chunks_exact(3).filter_map(move |t| {
            let p = |i: u32| {
                self.mesh
                    .positions
                    .get(i as usize)
                    .map(|q| DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64))
            };
            Some([p(t[0])?, p(t[1])?, p(t[2])?])
        })
    }

    /// Normal keluar face `f` di dekat titik `p` (normal segitiga face yang
    /// pusatnya paling dekat ke `p`). Segitiga mesh SolidCraft berputar
    /// berlawanan jarum jam dilihat dari luar, jadi normalnya menghadap keluar.
    pub fn normal_near(&self, f: usize, p: DVec3) -> Option<DVec3> {
        self.face_triangles(f)
            .filter_map(|[a, b, c]| {
                let n = (b - a).cross(c - a).normalize_or_zero();
                (n != DVec3::ZERO).then(|| (((a + b + c) / 3.0 - p).length_squared(), n))
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map(|(_, n)| n)
    }

    /// Loop batas terluar face `f` (port `chain_face_boundary_points` OCCT).
    pub fn face_boundary(&self, f: usize) -> Vec<DVec3> {
        let lists: Vec<Vec<DVec3>> = self
            .faces
            .get(f)
            .map(|lf| {
                lf.edges
                    .iter()
                    .filter_map(|&e| self.edges.get(e))
                    .map(|e| e.points.clone())
                    .filter(|p| p.len() >= 2)
                    .collect()
            })
            .unwrap_or_default();
        chain_outer_loop(lists)
    }
}

/// Urutkan anggota rantai edge dan gabungkan polylinenya.
fn order_chain(
    chain: &[usize],
    polylines: &[Vec<DVec3>],
    q: &dyn Fn(DVec3) -> Key,
    shared: &dyn Fn(Key) -> bool,
) -> (Vec<usize>, Vec<DVec3>) {
    if chain.len() == 1 {
        return (chain.to_vec(), polylines[chain[0]].clone());
    }
    let ends = |e: usize| {
        let p = &polylines[e];
        (q(p[0]), q(p[p.len() - 1]))
    };
    // Mulai dari ujung bebas (rantai terbuka) atau anggota terkecil (tertutup).
    let mut count: HashMap<Key, usize> = HashMap::new();
    for &e in chain {
        let (a, b) = ends(e);
        *count.entry(a).or_default() += 1;
        *count.entry(b).or_default() += 1;
    }
    let mut start_edge = chain.iter().copied().min().unwrap_or(chain[0]);
    let mut start_key = ends(start_edge).0;
    // Rantai tertutup: mulai di vertex yang juga dipakai edge lain, bila ada.
    if let Some((e, k)) = chain.iter().find_map(|&e| {
        let (a, b) = ends(e);
        if shared(a) {
            Some((e, a))
        } else if shared(b) {
            Some((e, b))
        } else {
            None
        }
    }) {
        (start_edge, start_key) = (e, k);
    }
    for &e in chain {
        let (a, b) = ends(e);
        if count.get(&a) == Some(&1) {
            (start_edge, start_key) = (e, a);
            break;
        }
        if count.get(&b) == Some(&1) {
            (start_edge, start_key) = (e, b);
            break;
        }
    }
    let mut used = vec![false; chain.len()];
    let mut members = Vec::with_capacity(chain.len());
    let mut points: Vec<DVec3> = Vec::new();
    let mut cur_key = start_key;
    let mut next = Some(start_edge);
    while let Some(e) = next {
        let Some(slot) = chain.iter().position(|&c| c == e) else {
            break;
        };
        used[slot] = true;
        members.push(e);
        let (a, b) = ends(e);
        let mut pts = polylines[e].clone();
        if a != cur_key && b == cur_key {
            pts.reverse();
        }
        cur_key = if a == cur_key { b } else { a };
        if points.is_empty() {
            points = pts;
        } else {
            points.extend(pts.into_iter().skip(1));
        }
        next = chain
            .iter()
            .enumerate()
            .find(|(i, &c)| {
                !used[*i] && {
                    let (x, y) = ends(c);
                    x == cur_key || y == cur_key
                }
            })
            .map(|(_, &c)| c);
    }
    // Anggota yang tak tersambung (seharusnya tidak ada) tetap dicatat.
    for (i, &e) in chain.iter().enumerate() {
        if !used[i] {
            members.push(e);
        }
    }
    (members, points)
}

/// Susun polyline edge jadi loop tersambung dan ambil yang bbox-nya
/// terbesar (loop luar) — port `chain_face_boundary_points` OCCT.
pub(crate) fn chain_outer_loop(lists: Vec<Vec<DVec3>>) -> Vec<DVec3> {
    if lists.len() <= 1 {
        return lists.into_iter().next().unwrap_or_default();
    }
    const EPS: f64 = 1e-4;
    let mut used = vec![false; lists.len()];
    let mut loops: Vec<Vec<DVec3>> = Vec::new();
    for start in 0..lists.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut chain = lists[start].clone();
        while let Some(&tail) = chain.last() {
            let mut found = false;
            for (i, pts) in lists.iter().enumerate() {
                if used[i] {
                    continue;
                }
                let (s, e) = (pts[0], pts[pts.len() - 1]);
                if (s - tail).length_squared() < EPS {
                    chain.extend(pts.iter().skip(1).copied());
                } else if (e - tail).length_squared() < EPS {
                    chain.extend(pts.iter().rev().skip(1).copied());
                } else {
                    continue;
                }
                used[i] = true;
                found = true;
                break;
            }
            if !found
                || (chain.len() > 2 && (chain[0] - chain[chain.len() - 1]).length_squared() < EPS)
            {
                break;
            }
        }
        if chain.len() > 1 && (chain[0] - chain[chain.len() - 1]).length_squared() < EPS {
            chain.pop();
        }
        loops.push(chain);
    }
    let diag = |pts: &[DVec3]| {
        let mut it = pts.iter();
        let Some(&first) = it.next() else { return 0.0 };
        let (lo, hi) = it.fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        (hi - lo).length()
    };
    loops
        .into_iter()
        .max_by(|a, b| diag(a).total_cmp(&diag(b)))
        .unwrap_or_default()
}
