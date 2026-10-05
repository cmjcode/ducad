//! Laporan hasil simulasi dan medan nodal untuk pewarnaan mesh render.

use serde::{Deserialize, Serialize};

use crate::mesh::spatial::CellGrid;
use crate::mesh::{HexMesh, TetMesh, NO_INDEX};
use crate::setup::MeshKind;

/// Gaya reaksi total satu fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reaction {
    pub fixture_id: String,
    /// Gaya yang diberikan tumpuan kepada body, N.
    pub force_n: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshStats {
    pub kind: MeshKind,
    pub nodes: usize,
    pub elements: usize,
    /// Ukuran sel rata-rata (akar pangkat tiga volume sel), mm.
    pub cell_mm: f64,
    /// Volume mesh setelah koreksi, mm³.
    pub volume_mm3: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolverStats {
    pub iterations: usize,
    /// Residual relatif akhir `‖r‖/‖b‖`.
    pub residual: f64,
    /// Jumlah DOF bebas (tidak dieliminasi).
    pub dofs: usize,
}

/// Ringkasan studi statik. `nodal_field` tidak ikut diserialisasi (besar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimReport {
    pub max_von_mises_mpa: f64,
    /// Posisi node dengan von Mises maksimum, mm.
    pub location: [f64; 3],
    pub max_displacement_mm: f64,
    /// `yield / max von Mises`, dibatasi [`SAFETY_FACTOR_CAP`]; 0 bila
    /// material tidak punya tegangan luluh.
    pub safety_factor: f64,
    pub reactions: Vec<Reaction>,
    pub mesh_stats: MeshStats,
    pub solver_stats: SolverStats,
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub nodal_field: Option<Field>,
}

/// Batas atas faktor keamanan yang dilaporkan (model nyaris tanpa tegangan),
/// agar nilainya tetap bisa ditulis sebagai JSON.
pub const SAFETY_FACTOR_CAP: f64 = 1.0e6;

/// Nilai medan di satu titik.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldSample {
    pub von_mises_mpa: f64,
    pub displacement: [f64; 3],
}

/// Pencari lokasi: memetakan titik ke node + bobot interpolasi.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Locator {
    /// Grid hex voxel (interpolasi trilinear).
    Grid {
        origin: [f64; 3],
        cell: [f64; 3],
        dims: [usize; 3],
        elems: Vec<[u32; 8]>,
        elem_of_cell: Vec<u32>,
    },
    /// Mesh Tet10 (fungsi bentuk kuadratik) dengan grid pencarian.
    Tets {
        elems: Vec<[u32; 10]>,
        grid: CellGrid,
    },
}

/// Hingga sepuluh node + bobot.
pub(crate) type Stencil = ([u32; 10], [f64; 10], usize);

impl Locator {
    pub(crate) fn hex(mesh: &HexMesh) -> Locator {
        Locator::Grid {
            origin: mesh.origin,
            cell: mesh.cell,
            dims: mesh.dims,
            elems: mesh.elems.clone(),
            elem_of_cell: mesh.elem_of_cell.clone(),
        }
    }

    pub(crate) fn tet(mesh: &TetMesh) -> Locator {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for p in &mesh.nodes {
            for a in 0..3 {
                min[a] = min[a].min(p[a]);
                max[a] = max[a].max(p[a]);
            }
        }
        let volume: f64 = (0..3).map(|a| (max[a] - min[a]).max(1e-9)).product();
        let cell = (volume / mesh.elems.len().max(1) as f64).cbrt().max(1e-9);
        let mut grid = CellGrid::new(min, max, cell, 500_000);
        let mut pairs = Vec::new();
        for (e, conn) in mesh.elems.iter().enumerate() {
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for &n in conn {
                for a in 0..3 {
                    lo[a] = lo[a].min(mesh.nodes[n as usize][a]);
                    hi[a] = hi[a].max(mesh.nodes[n as usize][a]);
                }
            }
            grid.push_box(lo, hi, e as u32, &mut pairs);
        }
        grid.fill(pairs);
        Locator::Tets {
            elems: mesh.elems.clone(),
            grid,
        }
    }

    /// Node dan bobot interpolasi untuk titik `p`; `None` bila `p` jauh dari
    /// mesh (pemanggil lalu memakai node terdekat).
    pub(crate) fn stencil(&self, nodes: &[[f64; 3]], p: [f64; 3]) -> Option<Stencil> {
        match self {
            Locator::Grid {
                origin,
                cell,
                dims,
                elems,
                elem_of_cell,
            } => {
                let index = |c: [usize; 3]| c[0] + dims[0] * (c[1] + dims[1] * c[2]);
                let mut center = [0usize; 3];
                for a in 0..3 {
                    let t = ((p[a] - origin[a]) / cell[a]).floor();
                    let max = dims[a].saturating_sub(1) as f64;
                    center[a] = if t.is_finite() {
                        t.clamp(0.0, max) as usize
                    } else {
                        0
                    };
                }
                let mut found = elem_of_cell
                    .get(index(center))
                    .is_some_and(|&e| e != NO_INDEX)
                    .then_some(center);
                if found.is_none() {
                    for radius in [1usize, 2, 4] {
                        let mut best: Option<(f64, [usize; 3])> = None;
                        for k in
                            center[2].saturating_sub(radius)..=(center[2] + radius).min(dims[2] - 1)
                        {
                            for j in center[1].saturating_sub(radius)
                                ..=(center[1] + radius).min(dims[1] - 1)
                            {
                                for i in center[0].saturating_sub(radius)
                                    ..=(center[0] + radius).min(dims[0] - 1)
                                {
                                    let c = [i, j, k];
                                    if elem_of_cell[index(c)] == NO_INDEX {
                                        continue;
                                    }
                                    // Jarak kuadrat titik ke kotak sel.
                                    let mut d2 = 0.0;
                                    for a in 0..3 {
                                        let lo = origin[a] + c[a] as f64 * cell[a];
                                        let d = (lo - p[a]).max(p[a] - lo - cell[a]).max(0.0);
                                        d2 += d * d;
                                    }
                                    if best.is_none_or(|b| d2 < b.0) {
                                        best = Some((d2, c));
                                    }
                                }
                            }
                        }
                        if let Some((_, c)) = best {
                            found = Some(c);
                            break;
                        }
                    }
                }
                let c = found?;
                let elem = elems[elem_of_cell[index(c)] as usize];
                let mut t = [0.0; 3];
                for a in 0..3 {
                    let lo = origin[a] + c[a] as f64 * cell[a];
                    t[a] = ((p[a] - lo) / cell[a]).clamp(0.0, 1.0);
                }
                let mut ids = [0u32; 10];
                let mut weights = [0.0; 10];
                for (n, &node) in elem.iter().enumerate() {
                    let mut w = 1.0;
                    for a in 0..3 {
                        w *= if (n >> a) & 1 == 1 { t[a] } else { 1.0 - t[a] };
                    }
                    ids[n] = node;
                    weights[n] = w;
                }
                Some((ids, weights, 8))
            }
            Locator::Tets { elems, grid } => {
                let center = grid.cell_of(p);
                // (min koordinat barisentrik, elemen, barisentrik)
                let mut best: Option<(f64, usize, [f64; 4])> = None;
                'search: for radius in 0..=2usize {
                    for k in center[2].saturating_sub(radius)
                        ..=(center[2] + radius).min(grid.dims[2] - 1)
                    {
                        for j in center[1].saturating_sub(radius)
                            ..=(center[1] + radius).min(grid.dims[1] - 1)
                        {
                            for i in center[0].saturating_sub(radius)
                                ..=(center[0] + radius).min(grid.dims[0] - 1)
                            {
                                for &e in grid.items([i, j, k]) {
                                    let conn = elems[e as usize];
                                    let x = [
                                        nodes[conn[0] as usize],
                                        nodes[conn[1] as usize],
                                        nodes[conn[2] as usize],
                                        nodes[conn[3] as usize],
                                    ];
                                    let Some(l) = barycentric(&x, p) else {
                                        continue;
                                    };
                                    let low = l[0].min(l[1]).min(l[2]).min(l[3]);
                                    if best.is_none_or(|b| low > b.0) {
                                        best = Some((low, e as usize, l));
                                    }
                                }
                            }
                        }
                    }
                    if best.is_some_and(|b| b.0 >= -1e-9) {
                        break 'search;
                    }
                }
                let (_, e, mut l) = best?;
                // Titik sedikit di luar: jepit ke dalam elemen.
                let mut sum = 0.0;
                for v in l.iter_mut() {
                    *v = v.max(0.0);
                    sum += *v;
                }
                if sum <= 0.0 {
                    return None;
                }
                for v in l.iter_mut() {
                    *v /= sum;
                }
                Some((elems[e], crate::element::tet10::shape(l), 10))
            }
        }
    }
}

/// Koordinat barisentrik `p` terhadap tet lurus `x`.
fn barycentric(x: &[[f64; 3]; 4], p: [f64; 3]) -> Option<[f64; 4]> {
    let m = [
        [x[1][0] - x[0][0], x[2][0] - x[0][0], x[3][0] - x[0][0]],
        [x[1][1] - x[0][1], x[2][1] - x[0][1], x[3][1] - x[0][1]],
        [x[1][2] - x[0][2], x[2][2] - x[0][2], x[3][2] - x[0][2]],
    ];
    let inv = crate::linalg::invert3(&m)?;
    let d = [p[0] - x[0][0], p[1] - x[0][1], p[2] - x[0][2]];
    let l1 = inv[0][0] * d[0] + inv[0][1] * d[1] + inv[0][2] * d[2];
    let l2 = inv[1][0] * d[0] + inv[1][1] * d[1] + inv[1][2] * d[2];
    let l3 = inv[2][0] * d[0] + inv[2][1] * d[1] + inv[2][2] * d[2];
    Some([1.0 - l1 - l2 - l3, l1, l2, l3])
}

fn nearest_node(nodes: &[[f64; 3]], p: [f64; 3]) -> usize {
    let mut best = (f64::INFINITY, 0usize);
    for (i, q) in nodes.iter().enumerate() {
        let d2 = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2);
        if d2 < best.0 {
            best = (d2, i);
        }
    }
    best.1
}

/// Medan nodal perpindahan + von Mises dengan interpolasi di dalam elemen.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub nodes: Vec<[f64; 3]>,
    pub displacement: Vec<[f64; 3]>,
    pub von_mises: Vec<f64>,
    locator: Locator,
}

impl Field {
    pub(crate) fn new(
        nodes: Vec<[f64; 3]>,
        locator: Locator,
        displacement: Vec<[f64; 3]>,
        von_mises: Vec<f64>,
    ) -> Field {
        Field {
            nodes,
            displacement,
            von_mises,
            locator,
        }
    }

    /// Nilai medan di titik `p` (mm): interpolasi di elemen yang memuat `p`
    /// (trilinear untuk hex, kuadratik untuk Tet10); titik sedikit di luar
    /// mesh memakai elemen terdekat (koordinat lokal dijepit), dan node
    /// terdekat bila jauh dari mesh.
    pub fn sample(&self, p: [f64; 3]) -> FieldSample {
        if self.nodes.is_empty() || p.iter().any(|v| !v.is_finite()) {
            return FieldSample {
                von_mises_mpa: 0.0,
                displacement: [0.0; 3],
            };
        }
        if let Some((ids, weights, count)) = self.locator.stencil(&self.nodes, p) {
            let mut out = FieldSample {
                von_mises_mpa: 0.0,
                displacement: [0.0; 3],
            };
            for n in 0..count {
                let node = ids[n];
                let w = weights[n];
                out.von_mises_mpa += w * self.von_mises[node as usize];
                for a in 0..3 {
                    out.displacement[a] += w * self.displacement[node as usize][a];
                }
            }
            return out;
        }
        // Jauh dari mesh: node terdekat (pencarian linier, jarang terjadi).
        let i = nearest_node(&self.nodes, p);
        FieldSample {
            von_mises_mpa: self.von_mises[i],
            displacement: self.displacement[i],
        }
    }
}

/// Medan skalar nodal (suhu, bentuk mode per komponen, dst.).
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarField {
    pub nodes: Vec<[f64; 3]>,
    pub values: Vec<f64>,
    locator: Locator,
}

impl ScalarField {
    pub(crate) fn new(nodes: Vec<[f64; 3]>, locator: Locator, values: Vec<f64>) -> ScalarField {
        ScalarField {
            nodes,
            values,
            locator,
        }
    }

    /// Nilai di titik `p`, dengan aturan yang sama seperti [`Field::sample`].
    pub fn sample(&self, p: [f64; 3]) -> f64 {
        if self.nodes.is_empty() || p.iter().any(|v| !v.is_finite()) {
            return 0.0;
        }
        match self.locator.stencil(&self.nodes, p) {
            Some((ids, weights, count)) => (0..count)
                .map(|n| weights[n] * self.values[ids[n] as usize])
                .sum(),
            None => self.values[nearest_node(&self.nodes, p)],
        }
    }
}

/// Medan vektor nodal (bentuk mode getar atau tekuk).
#[derive(Debug, Clone, PartialEq)]
pub struct VectorField {
    pub nodes: Vec<[f64; 3]>,
    pub vectors: Vec<[f64; 3]>,
    locator: Locator,
}

impl VectorField {
    pub(crate) fn new(
        nodes: Vec<[f64; 3]>,
        locator: Locator,
        vectors: Vec<[f64; 3]>,
    ) -> VectorField {
        VectorField {
            nodes,
            vectors,
            locator,
        }
    }

    /// Vektor di titik `p`, dengan aturan yang sama seperti [`Field::sample`].
    pub fn sample(&self, p: [f64; 3]) -> [f64; 3] {
        if self.nodes.is_empty() || p.iter().any(|v| !v.is_finite()) {
            return [0.0; 3];
        }
        match self.locator.stencil(&self.nodes, p) {
            Some((ids, weights, count)) => {
                let mut out = [0.0; 3];
                for n in 0..count {
                    for a in 0..3 {
                        out[a] += weights[n] * self.vectors[ids[n] as usize][a];
                    }
                }
                out
            }
            None => self.vectors[nearest_node(&self.nodes, p)],
        }
    }
}
