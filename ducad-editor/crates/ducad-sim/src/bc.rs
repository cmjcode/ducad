//! Syarat batas: mengubah fixture dan beban pada face B-rep menjadi kekangan
//! DOF dan gaya nodal pada mesh voxel.
//!
//! Beban diintegralkan di atas sampel permukaan *asli* (potongan segitiga),
//! lalu tiap sampel "disetor" ke empat node quad batas terdekat yang bertag
//! face yang sama dengan bobot bilinear. Hasilnya luas nodal `A_n` dan
//! vektor luas nodal `a_n` (normal keluar × luas). Gaya nodal dihitung dari
//! besaran nodal itu, sehingga resultan gaya (dan momen untuk torsi/remote)
//! pada mesh tepat sama dengan yang diminta.

use std::collections::BTreeMap;

use crate::assemble::Penalty;
use crate::linalg::{add, cross, dot, invert3, norm, normalize, scale, sub, V3};
use crate::mesh::tet::TetModel;
use crate::mesh::voxel::VoxelModel;
use crate::mesh::{HexMesh, NO_INDEX};
use crate::setup::{FixtureKind, LoadKind, ResolvedSetup};
use crate::{ElasticMaterial, SimError};

/// Kekangan tambahan tingkat node: perpindahan node `node` pada sumbu `axis`
/// (0 = X, 1 = Y, 2 = Z) dinolkan dengan eliminasi tepat. Dipakai untuk
/// tumpuan yang tidak bisa dinyatakan lewat face (mis. tumpuan sederhana
/// sepanjang tepi) dan oleh benchmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeConstraint {
    pub node: u32,
    pub axis: u8,
}

/// Besaran face yang sudah disetor ke node.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FaceNodal {
    /// Node terurut menaik.
    pub nodes: Vec<u32>,
    /// Luas nodal, mm².
    pub area: Vec<f64>,
    /// Vektor luas nodal (normal keluar × luas), mm².
    pub area_vec: Vec<V3>,
}

impl FaceNodal {
    pub fn total_area(&self) -> f64 {
        self.area.iter().sum()
    }

    fn area_vec_of(&self, node: u32) -> Option<V3> {
        self.nodes
            .binary_search(&node)
            .ok()
            .map(|i| self.area_vec[i])
    }
}

/// Syarat batas siap rakit.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryConditions {
    /// Per DOF: dieliminasi atau tidak.
    pub fixed: Vec<bool>,
    /// Per node: indeks fixture `Fixed` pemiliknya, `NO_INDEX` bila tidak ada.
    pub fixed_owner: Vec<u32>,
    pub penalties: Vec<Penalty>,
    /// Gaya luar per DOF (N), termasuk pada DOF yang dikekang.
    pub force: Vec<f64>,
    pub warnings: Vec<String>,
}

fn invalid(msg: String) -> SimError {
    SimError::InvalidSetup(msg)
}

fn finite3(v: V3) -> bool {
    v.iter().all(|c| c.is_finite())
}

/// Antarmuka mesh untuk syarat batas, sehingga `build` tidak bergantung pada
/// jenis mesh (hex voxel atau Tet10).
pub trait BoundaryMesh {
    fn positions(&self) -> &[V3];
    /// Luas nodal dan vektor luas nodal face-face `faces`.
    fn face_nodal(
        &self,
        faces: &[u32],
        what: &str,
        warnings: &mut Vec<String>,
    ) -> Result<FaceNodal, SimError>;
    /// Node pada face-face `faces` beserta jumlah normal keluar × luas.
    fn fixture_nodes(&self, faces: &[u32]) -> BTreeMap<u32, V3>;
    /// Menambahkan gaya badan `unit × per_volume` (N/mm³) ke `force`.
    fn add_body_force(&self, unit: V3, per_volume: f64, force: &mut [f64]);
    /// `true` bila normal roller harus diambil dari normal sisi mesh
    /// (`fixture_nodes`), bukan dari vektor luas nodal. Pada segitiga
    /// kuadratik vektor luas konsisten di node sudut bernilai nol.
    fn prefer_facet_normals(&self) -> bool {
        false
    }
}

impl BoundaryMesh for VoxelModel {
    fn positions(&self) -> &[V3] {
        &self.mesh.nodes
    }

    fn face_nodal(
        &self,
        faces: &[u32],
        what: &str,
        warnings: &mut Vec<String>,
    ) -> Result<FaceNodal, SimError> {
        face_nodal(self, faces, what, warnings)
    }

    fn fixture_nodes(&self, faces: &[u32]) -> BTreeMap<u32, V3> {
        tagged_nodes(&self.mesh, faces)
    }

    fn add_body_force(&self, unit: V3, per_volume: f64, force: &mut [f64]) {
        let mesh = &self.mesh;
        let cell_volume = mesh.cell_volume();
        for (e, conn) in mesh.elems.iter().enumerate() {
            let f = scale(unit, mesh.weight[e] * cell_volume * per_volume / 8.0);
            for &n in conn {
                add_force(force, n, f);
            }
        }
    }
}

/// Kuadratur segitiga 6 titik (Dunavant derajat 4): (λ0, λ1, λ2, bobot),
/// bobot berjumlah 1.
const TRI6: [[f64; 4]; 6] = [
    [
        0.108_103_018_168_070,
        0.445_948_490_915_965,
        0.445_948_490_915_965,
        0.223_381_589_678_011,
    ],
    [
        0.445_948_490_915_965,
        0.108_103_018_168_070,
        0.445_948_490_915_965,
        0.223_381_589_678_011,
    ],
    [
        0.445_948_490_915_965,
        0.445_948_490_915_965,
        0.108_103_018_168_070,
        0.223_381_589_678_011,
    ],
    [
        0.816_847_572_980_459,
        0.091_576_213_509_771,
        0.091_576_213_509_771,
        0.109_951_743_655_322,
    ],
    [
        0.091_576_213_509_771,
        0.816_847_572_980_459,
        0.091_576_213_509_771,
        0.109_951_743_655_322,
    ],
    [
        0.091_576_213_509_771,
        0.091_576_213_509_771,
        0.816_847_572_980_459,
        0.109_951_743_655_322,
    ],
];

/// Luas nodal konsisten `∫ N_a dA` dan vektor luas `∫ N_a n dA` segitiga
/// kuadratik 6-node (sudut 0–2, tengah tepi 01, 12, 20).
pub fn tri6_nodal(x: &[V3; 6]) -> ([f64; 6], [V3; 6]) {
    let mut area = [0.0; 6];
    let mut area_vec = [[0.0; 3]; 6];
    for q in TRI6 {
        let l = [q[0], q[1], q[2]];
        let n = [
            l[0] * (2.0 * l[0] - 1.0),
            l[1] * (2.0 * l[1] - 1.0),
            l[2] * (2.0 * l[2] - 1.0),
            4.0 * l[0] * l[1],
            4.0 * l[1] * l[2],
            4.0 * l[2] * l[0],
        ];
        // Turunan terhadap (ξ, η) = (λ1, λ2), λ0 = 1 − ξ − η.
        let d_xi = [
            -(4.0 * l[0] - 1.0),
            4.0 * l[1] - 1.0,
            0.0,
            4.0 * (l[0] - l[1]),
            4.0 * l[2],
            -4.0 * l[2],
        ];
        let d_eta = [
            -(4.0 * l[0] - 1.0),
            0.0,
            4.0 * l[2] - 1.0,
            -4.0 * l[1],
            4.0 * l[1],
            4.0 * (l[0] - l[2]),
        ];
        let mut tx = [0.0; 3];
        let mut ty = [0.0; 3];
        for a in 0..6 {
            tx = add(tx, scale(x[a], d_xi[a]));
            ty = add(ty, scale(x[a], d_eta[a]));
        }
        let normal = scale(cross(tx, ty), 0.5 * q[3]);
        let da = norm(normal);
        for a in 0..6 {
            area[a] += n[a] * da;
            area_vec[a] = add(area_vec[a], scale(normal, n[a]));
        }
    }
    (area, area_vec)
}

impl BoundaryMesh for TetModel {
    fn positions(&self) -> &[V3] {
        &self.mesh.nodes
    }

    fn face_nodal(
        &self,
        faces: &[u32],
        what: &str,
        _warnings: &mut Vec<String>,
    ) -> Result<FaceNodal, SimError> {
        let mut sorted = faces.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        let mut acc: BTreeMap<u32, (f64, V3)> = BTreeMap::new();
        for &face in &sorted {
            if self.surface_tags.binary_search(&face).is_err() {
                return Err(invalid(format!(
                    "{what}: face index {face} does not exist on the body surface"
                )));
            }
            for f in self.mesh.faces_of(face) {
                let x = f.nodes.map(|n| self.mesh.nodes[n as usize]);
                let (area, area_vec) = tri6_nodal(&x);
                for a in 0..6 {
                    let slot = acc.entry(f.nodes[a]).or_insert((0.0, [0.0; 3]));
                    slot.0 += area[a];
                    slot.1 = add(slot.1, area_vec[a]);
                }
            }
        }
        let mut out = FaceNodal::default();
        for (node, (area, av)) in acc {
            out.nodes.push(node);
            out.area.push(area);
            out.area_vec.push(av);
        }
        if out.nodes.is_empty() || out.total_area() <= 0.0 {
            return Err(SimError::MeshTooCoarse {
                cell_mm: self.size_mm,
                elements: self.mesh.elems.len(),
                detail: format!("{what}: the selected faces are smaller than the mesh size and captured no mesh nodes"),
            });
        }
        Ok(out)
    }

    fn fixture_nodes(&self, faces: &[u32]) -> BTreeMap<u32, V3> {
        let mut map: BTreeMap<u32, V3> = BTreeMap::new();
        for &face in faces {
            for f in self.mesh.faces_of(face) {
                let p = [
                    self.mesh.nodes[f.nodes[0] as usize],
                    self.mesh.nodes[f.nodes[1] as usize],
                    self.mesh.nodes[f.nodes[2] as usize],
                ];
                let n = scale(cross(sub(p[1], p[0]), sub(p[2], p[0])), 0.5);
                for node in f.nodes {
                    let slot = map.entry(node).or_insert([0.0; 3]);
                    *slot = add(*slot, n);
                }
            }
        }
        map
    }

    fn add_body_force(&self, unit: V3, per_volume: f64, force: &mut [f64]) {
        let mut volumes = [0.0; 10];
        for (e, conn) in self.mesh.elems.iter().enumerate() {
            crate::element::tet10::nodal_volume(&self.mesh.elem_coords(e), &mut volumes);
            for (a, &n) in conn.iter().enumerate() {
                add_force(force, n, scale(unit, volumes[a] * per_volume));
            }
        }
    }

    fn prefer_facet_normals(&self) -> bool {
        true
    }
}

/// Menyetor sampel face-face `faces` ke node mesh.
pub fn face_nodal(
    model: &VoxelModel,
    faces: &[u32],
    what: &str,
    warnings: &mut Vec<String>,
) -> Result<FaceNodal, SimError> {
    let mesh = &model.mesh;
    let mut sorted = faces.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut acc: BTreeMap<u32, (f64, V3)> = BTreeMap::new();
    for &face in &sorted {
        let samples = model.samples.of_face(face);
        if samples.is_empty() {
            return Err(invalid(format!(
                "{what}: face index {face} does not exist on the body surface"
            )));
        }
        let own_quads = !mesh.quads_of_face(face).is_empty();
        if !own_quads {
            warnings.push(format!(
                "{what}: face {face} is smaller than a mesh cell; it was mapped to the nearest mesh nodes"
            ));
        }
        let filter = own_quads.then_some(face);
        for &s in samples {
            let s = s as usize;
            let Some(hit) = mesh.nearest_quad(model.samples.pos[s], filter) else {
                continue;
            };
            let av = model.samples.area_vec[s];
            let area = norm(av);
            for q in 0..4 {
                let w = hit.weights[q];
                if w == 0.0 {
                    continue;
                }
                let slot = acc.entry(hit.nodes[q]).or_insert((0.0, [0.0; 3]));
                slot.0 += w * area;
                slot.1 = add(slot.1, scale(av, w));
            }
        }
    }
    let mut out = FaceNodal::default();
    for (node, (area, av)) in acc {
        out.nodes.push(node);
        out.area.push(area);
        out.area_vec.push(av);
    }
    if out.nodes.is_empty() || out.total_area() <= 0.0 {
        return Err(SimError::MeshTooCoarse {
            cell_mm: mesh.cell_volume().cbrt(),
            elements: mesh.elems.len(),
            detail: format!("{what}: the selected faces captured no mesh nodes"),
        });
    }
    Ok(out)
}

/// Node quad bertag `faces` + jumlah normal quad per node.
fn tagged_nodes(mesh: &HexMesh, faces: &[u32]) -> BTreeMap<u32, V3> {
    let mut map: BTreeMap<u32, V3> = BTreeMap::new();
    for &face in faces {
        for &(_, e, dir) in mesh.quads_of_face(face) {
            let n = scale(
                HexMesh::quad_normal(dir as usize),
                mesh.quad_area(dir as usize),
            );
            for node in mesh.quad_nodes(e as usize, dir as usize) {
                let slot = map.entry(node).or_insert([0.0; 3]);
                *slot = add(*slot, n);
            }
        }
    }
    map
}

fn add_force(force: &mut [f64], node: u32, f: V3) {
    let base = 3 * node as usize;
    for a in 0..3 {
        force[base + a] += f[a];
    }
}

/// Titik berat luas nodal.
fn centroid(nodes: &[V3], fnod: &FaceNodal) -> V3 {
    let mut c = [0.0; 3];
    for (i, &n) in fnod.nodes.iter().enumerate() {
        c = add(c, scale(nodes[n as usize], fnod.area[i]));
    }
    scale(c, 1.0 / fnod.total_area())
}

fn apply_face_load(
    nodes: &[V3],
    fnod: &FaceNodal,
    id: &str,
    kind: &LoadKind,
    force: &mut [f64],
) -> Result<(), SimError> {
    let total_area = fnod.total_area();
    match *kind {
        LoadKind::Force { newton } => {
            if !finite3(newton) {
                return Err(invalid(format!("load '{id}': force must be finite")));
            }
            for (i, &n) in fnod.nodes.iter().enumerate() {
                add_force(force, n, scale(newton, fnod.area[i] / total_area));
            }
        }
        LoadKind::Pressure { mpa } => {
            if !mpa.is_finite() {
                return Err(invalid(format!("load '{id}': pressure must be finite")));
            }
            // Positif = menekan body: berlawanan normal keluar.
            for (i, &n) in fnod.nodes.iter().enumerate() {
                add_force(force, n, scale(fnod.area_vec[i], -mpa));
            }
        }
        LoadKind::Torque {
            axis_point,
            axis_dir,
            newton_mm,
        } => {
            let axis = normalize(axis_dir)
                .filter(|_| finite3(axis_point) && newton_mm.is_finite())
                .ok_or_else(|| {
                    invalid(format!(
                        "load '{id}': torque needs a non-zero axis_dir and finite values"
                    ))
                })?;
            // Kopel murni: lengan diukur dari titik berat face sehingga
            // resultan gayanya nol; `axis_point` tidak mengubah hasil.
            let c = centroid(nodes, fnod);
            let mut arms = Vec::with_capacity(fnod.nodes.len());
            let mut polar = 0.0;
            for (i, &n) in fnod.nodes.iter().enumerate() {
                let r = sub(nodes[n as usize], c);
                let rho = sub(r, scale(axis, dot(r, axis)));
                polar += fnod.area[i] * dot(rho, rho);
                arms.push(rho);
            }
            if polar <= 1e-12 * total_area * total_area {
                return Err(invalid(format!(
                    "load '{id}': the selected faces have no lever arm about the torque axis"
                )));
            }
            let k = newton_mm / polar;
            for (i, &n) in fnod.nodes.iter().enumerate() {
                add_force(force, n, scale(cross(axis, arms[i]), k * fnod.area[i]));
            }
        }
        LoadKind::Bearing { newton } => {
            let magnitude = norm(newton);
            let dir = normalize(newton).ok_or_else(|| {
                invalid(format!(
                    "load '{id}': bearing force must be non-zero and finite"
                ))
            })?;
            // Tekanan cosinus pada separuh permukaan yang ditekan beban:
            // p ∝ max(0, −n·F̂), bekerja sepanjang −n.
            let mut parts = Vec::with_capacity(fnod.nodes.len());
            let mut resultant = [0.0; 3];
            let mut weight_sum = 0.0;
            for i in 0..fnod.nodes.len() {
                let g = match normalize(fnod.area_vec[i]) {
                    Some(nrm) => scale(nrm, -fnod.area[i] * (-dot(nrm, dir)).max(0.0)),
                    None => [0.0; 3],
                };
                resultant = add(resultant, g);
                weight_sum += norm(g);
                parts.push(g);
            }
            let along = dot(resultant, dir);
            if along.is_nan() || along <= 1e-12 * total_area {
                return Err(invalid(format!(
                    "load '{id}': no part of the selected faces faces the bearing load direction"
                )));
            }
            let s = magnitude / along;
            // Sisa lateral (face tidak simetris) dibagi sebanding bobot agar
            // resultan tepat sama dengan gaya yang diminta.
            let rest = sub(newton, scale(resultant, s));
            for (i, &n) in fnod.nodes.iter().enumerate() {
                let share = norm(parts[i]) / weight_sum;
                add_force(force, n, add(scale(parts[i], s), scale(rest, share)));
            }
        }
        LoadKind::Remote { point, newton } => {
            if !(finite3(point) && finite3(newton)) {
                return Err(invalid(format!(
                    "load '{id}': remote point and force must be finite"
                )));
            }
            // Gaya merata + medan traksi linier ω × r yang menghasilkan momen
            // M = (P − c) × F terhadap titik berat face.
            let c = centroid(nodes, fnod);
            let moment = cross(sub(point, c), newton);
            let mut inertia = [[0.0; 3]; 3];
            for (i, &n) in fnod.nodes.iter().enumerate() {
                let r = sub(nodes[n as usize], c);
                let rr = dot(r, r);
                for a in 0..3 {
                    for b in 0..3 {
                        let delta = if a == b { rr } else { 0.0 };
                        inertia[a][b] += fnod.area[i] * (delta - r[a] * r[b]);
                    }
                }
            }
            let omega = if norm(moment) == 0.0 {
                [0.0; 3]
            } else {
                // Regularisasi kecil untuk face yang nodenya segaris.
                let trace = inertia[0][0] + inertia[1][1] + inertia[2][2];
                for a in 0..3 {
                    inertia[a][a] += 1e-9 * trace;
                }
                let inv = invert3(&inertia).ok_or_else(|| {
                    invalid(format!(
                        "load '{id}': the selected faces are too small to carry the remote moment"
                    ))
                })?;
                [
                    dot(inv[0], moment),
                    dot(inv[1], moment),
                    dot(inv[2], moment),
                ]
            };
            for (i, &n) in fnod.nodes.iter().enumerate() {
                let r = sub(nodes[n as usize], c);
                let f = add(scale(newton, 1.0 / total_area), cross(omega, r));
                add_force(force, n, scale(f, fnod.area[i]));
            }
        }
        LoadKind::Gravity { .. } => {}
    }
    Ok(())
}

/// Membangun kekangan dan vektor gaya dari setup.
pub fn build(
    model: &dyn BoundaryMesh,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    extra: &[NodeConstraint],
) -> Result<BoundaryConditions, SimError> {
    let positions = model.positions();
    let nn = positions.len();
    let mut bc = BoundaryConditions {
        fixed: vec![false; 3 * nn],
        fixed_owner: vec![NO_INDEX; nn],
        penalties: Vec::new(),
        force: vec![0.0; 3 * nn],
        warnings: Vec::new(),
    };

    for (fi, fixture) in setup.fixtures.iter().enumerate() {
        let what = format!("fixture '{}'", fixture.id);
        if fixture.faces.is_empty() {
            return Err(invalid(format!("{what}: no faces selected")));
        }
        let fnod = model.face_nodal(&fixture.faces, &what, &mut bc.warnings)?;
        let mut faces = fixture.faces.clone();
        faces.sort_unstable();
        faces.dedup();
        let mut nodes = model.fixture_nodes(&faces);
        // Face yang lebih kecil dari satu sel: pakai node hasil setoran.
        for &n in &fnod.nodes {
            nodes.entry(n).or_insert([0.0; 3]);
        }
        for (&node, &quad_normal) in &nodes {
            match fixture.kind {
                FixtureKind::Fixed => {
                    for a in 0..3 {
                        bc.fixed[3 * node as usize + a] = true;
                    }
                    if bc.fixed_owner[node as usize] == NO_INDEX {
                        bc.fixed_owner[node as usize] = fi as u32;
                    }
                }
                FixtureKind::Roller | FixtureKind::Symmetry => {
                    // Normal face asli (dari sampel) lebih tepat daripada
                    // normal quad yang selalu sejajar sumbu.
                    let dir = if model.prefer_facet_normals() {
                        normalize(quad_normal)
                    } else {
                        fnod.area_vec_of(node)
                            .and_then(normalize)
                            .or_else(|| normalize(quad_normal))
                    };
                    if let Some(dir) = dir {
                        bc.penalties.push(Penalty {
                            node,
                            dir,
                            owner: fi as u32,
                        });
                    }
                }
            }
        }
    }
    for c in extra {
        if c.node as usize >= nn || c.axis > 2 {
            return Err(invalid("node constraint index out of range".into()));
        }
        bc.fixed[3 * c.node as usize + c.axis as usize] = true;
    }
    // Node yang sudah terkunci penuh tidak butuh penalti.
    let fixed = &bc.fixed;
    bc.penalties.retain(|p| {
        let base = 3 * p.node as usize;
        !(fixed[base] && fixed[base + 1] && fixed[base + 2])
    });

    for load in &setup.loads {
        if let LoadKind::Gravity { g, dir } = load.kind {
            let unit = normalize(dir).filter(|_| g.is_finite()).ok_or_else(|| {
                invalid(format!(
                    "load '{}': gravity needs a non-zero dir and finite g",
                    load.id
                ))
            })?;
            if !(material.density_g_cm3.is_finite() && material.density_g_cm3 >= 0.0) {
                return Err(invalid("material density must be non-negative".into()));
            }
            // N = mm³ · g/cm³ · 1e-6 · m/s².
            let per_volume = material.density_g_cm3 * 1.0e-6 * g;
            model.add_body_force(unit, per_volume, &mut bc.force);
            continue;
        }
        let what = format!("load '{}'", load.id);
        if load.faces.is_empty() {
            return Err(invalid(format!("{what}: no faces selected")));
        }
        let fnod = model.face_nodal(&load.faces, &what, &mut bc.warnings)?;
        apply_face_load(positions, &fnod, &load.id, &load.kind, &mut bc.force)?;
    }
    if bc.force.iter().any(|f| !f.is_finite()) {
        return Err(invalid("load vector is not finite".into()));
    }
    Ok(bc)
}
