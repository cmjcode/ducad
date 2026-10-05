//! Alur studi statik: mesh → syarat batas → rakit → PCG → post-proses.
//! Inti alurnya tidak bergantung pada jenis mesh: hex voxel dan Tet10
//! sama-sama lewat [`MeshRef`], `ElementModel`, dan `BoundaryMesh`.

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::assemble::{assemble_stiffness, Assembled};
use crate::bc::{self, BoundaryConditions, BoundaryMesh, NodeConstraint};
use crate::element::{ElementModel, Hex8, HexModel, TetElements};
use crate::linalg::{dot, scale};
use crate::mesh::delaunay::TetError;
use crate::mesh::tet::{tetrahedralize, TetModel};
use crate::mesh::voxel::{voxelize, VoxelModel};
use crate::mesh::NO_INDEX;
use crate::post::{internal_forces, nodal_stress, von_mises};
use crate::report::{
    Field, Locator, MeshStats, Reaction, SimReport, SolverStats, SAFETY_FACTOR_CAP,
};
use crate::setup::{MeshKind, MeshSettings, ResolvedSetup};
use crate::solver::{solve_pcg, BlockJacobi3, CgOptions, Preconditioner, TwoLevel};
use crate::{CancelToken, ElasticMaterial, SimError, SurfaceMesh};

/// Awalan peringatan saat mesher tet gagal dan studi memakai mesh hex voxel.
pub const FALLBACK_WARNING: &str = "SIM_MESH_FALLBACK_HEX";

/// Mesh elemen hingga siap pakai: hex voxel atau Tet10.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum SimMesh {
    Hex(VoxelModel),
    Tet(TetModel),
}

impl SimMesh {
    pub fn kind(&self) -> MeshKind {
        self.as_ref().kind()
    }

    /// Posisi semua node.
    pub fn nodes(&self) -> &[[f64; 3]] {
        self.as_ref().nodes()
    }

    pub fn elements(&self) -> usize {
        self.as_ref().elements()
    }

    /// Node (terurut, unik) pada face B-rep `face`.
    pub fn face_nodes(&self, face: u32) -> Vec<u32> {
        match self {
            SimMesh::Hex(m) => m.mesh.face_nodes(face),
            SimMesh::Tet(m) => m.mesh.face_nodes(face),
        }
    }

    pub fn volume_mm3(&self) -> f64 {
        self.as_ref().volume()
    }

    pub fn warnings(&self) -> &[String] {
        self.as_ref().warnings()
    }

    pub(crate) fn as_ref(&self) -> MeshRef<'_> {
        match self {
            SimMesh::Hex(m) => MeshRef::Hex(m),
            SimMesh::Tet(m) => MeshRef::Tet(m),
        }
    }
}

/// Rujukan ke salah satu jenis mesh.
#[derive(Clone, Copy)]
pub(crate) enum MeshRef<'a> {
    Hex(&'a VoxelModel),
    Tet(&'a TetModel),
}

impl<'a> MeshRef<'a> {
    pub fn kind(self) -> MeshKind {
        match self {
            MeshRef::Hex(_) => MeshKind::Hex,
            MeshRef::Tet(_) => MeshKind::Tet,
        }
    }

    pub fn boundary(self) -> &'a dyn BoundaryMesh {
        match self {
            MeshRef::Hex(m) => m,
            MeshRef::Tet(m) => m,
        }
    }

    pub fn nodes(self) -> &'a [[f64; 3]] {
        match self {
            MeshRef::Hex(m) => &m.mesh.nodes,
            MeshRef::Tet(m) => &m.mesh.nodes,
        }
    }

    pub fn elements(self) -> usize {
        match self {
            MeshRef::Hex(m) => m.mesh.elems.len(),
            MeshRef::Tet(m) => m.mesh.elems.len(),
        }
    }

    pub fn volume(self) -> f64 {
        match self {
            MeshRef::Hex(m) => m.mesh.volume(),
            MeshRef::Tet(m) => m.mesh.volume(),
        }
    }

    pub fn warnings(self) -> &'a [String] {
        match self {
            MeshRef::Hex(m) => &m.warnings,
            MeshRef::Tet(m) => &m.warnings,
        }
    }

    /// Ukuran elemen yang dilaporkan, mm.
    pub fn cell_mm(self) -> f64 {
        match self {
            MeshRef::Hex(m) => m.mesh.cell_volume().cbrt(),
            MeshRef::Tet(m) => m.size_mm,
        }
    }

    /// Jarak node khas (untuk agregasi prekondisi).
    pub fn spacing(self) -> f64 {
        match self {
            MeshRef::Hex(m) => m.mesh.cell[0].min(m.mesh.cell[1]).min(m.mesh.cell[2]),
            MeshRef::Tet(m) => 0.5 * m.size_mm,
        }
    }

    pub fn locator(self) -> Locator {
        match self {
            MeshRef::Hex(m) => Locator::hex(&m.mesh),
            MeshRef::Tet(m) => Locator::tet(&m.mesh),
        }
    }

    pub fn element_model(
        self,
        material: &ElasticMaterial,
    ) -> Result<Box<dyn ElementModel + 'a>, SimError> {
        Ok(match self {
            MeshRef::Hex(m) => Box::new(HexModel::new(
                &m.mesh,
                material.young_mpa,
                material.poisson,
            )?),
            MeshRef::Tet(m) => Box::new(TetElements::new(
                &m.mesh,
                material.young_mpa,
                material.poisson,
            )?),
        })
    }

    pub fn stats(self) -> MeshStats {
        MeshStats {
            kind: self.kind(),
            nodes: self.nodes().len(),
            elements: self.elements(),
            cell_mm: self.cell_mm(),
            volume_mm3: self.volume(),
        }
    }
}

pub(crate) fn check_cancel(cancel: &CancelToken) -> Result<(), SimError> {
    if cancel.is_cancelled() {
        Err(SimError::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) fn validate_material(material: &ElasticMaterial) -> Result<(), SimError> {
    if !(material.yield_mpa.is_finite() && material.yield_mpa >= 0.0) {
        return Err(SimError::InvalidSetup(
            "material yield strength must be non-negative".into(),
        ));
    }
    if !(material.density_g_cm3.is_finite() && material.density_g_cm3 >= 0.0) {
        return Err(SimError::InvalidSetup(
            "material density must be non-negative".into(),
        ));
    }
    Ok(())
}

/// Face yang dibebani atau dikunci (diperhalus oleh mesher tet).
pub(crate) fn setup_faces(setup: &ResolvedSetup) -> Vec<u32> {
    let mut faces: Vec<u32> = setup
        .fixtures
        .iter()
        .flat_map(|f| f.faces.iter().copied())
        .chain(setup.loads.iter().flat_map(|l| l.faces.iter().copied()))
        .collect();
    faces.sort_unstable();
    faces.dedup();
    faces
}

/// Membuat mesh menurut `settings.kind`. Untuk `Tet`, kegagalan mesher
/// (termasuk panik internal) tidak menggagalkan studi: mesh hex voxel dipakai
/// dan peringatan berawalan [`FALLBACK_WARNING`] dicatat di mesh.
/// `refine_faces` = tag face yang perlu mesh lebih halus.
pub fn build_mesh(
    surface: &SurfaceMesh,
    settings: &MeshSettings,
    exact_volume_mm3: Option<f64>,
    refine_faces: &[u32],
    cancel: &CancelToken,
) -> Result<SimMesh, SimError> {
    if settings.kind == MeshKind::Hex {
        return Ok(SimMesh::Hex(voxelize(
            surface,
            settings,
            exact_volume_mm3,
            cancel,
        )?));
    }
    let attempt = catch_unwind(AssertUnwindSafe(|| {
        tetrahedralize(surface, settings, refine_faces, exact_volume_mm3, cancel)
    }));
    let reason = match attempt {
        Ok(Ok(model)) => return Ok(SimMesh::Tet(model)),
        Ok(Err(TetError::Cancelled)) => return Err(SimError::Cancelled),
        Ok(Err(TetError::Failed(reason))) => reason,
        Err(_) => "internal tet mesher error".to_string(),
    };
    let hex_settings = MeshSettings {
        kind: MeshKind::Hex,
        ..settings.clone()
    };
    let mut model = voxelize(surface, &hex_settings, exact_volume_mm3, cancel)?;
    model.warnings.insert(
        0,
        format!(
            "{FALLBACK_WARNING}: tet meshing failed ({reason}); a voxel hex mesh was used instead"
        ),
    );
    Ok(SimMesh::Hex(model))
}

/// Jumlah komponen terhubung mesh yang tidak punya kekangan sama sekali,
/// dan jumlah komponen seluruhnya (union-find lewat node elemen).
fn unconstrained_components(model: &dyn ElementModel, constrained: &[bool]) -> (usize, usize) {
    let nn = model.num_nodes();
    let mut parent: Vec<u32> = (0..nn as u32).collect();
    fn find(parent: &mut [u32], mut x: u32) -> u32 {
        while parent[x as usize] != x {
            parent[x as usize] = parent[parent[x as usize] as usize];
            x = parent[x as usize];
        }
        x
    }
    for e in 0..model.num_elems() {
        let conn = model.elem_nodes(e);
        let root = find(&mut parent, conn[0]);
        for &n in &conn[1..] {
            let r = find(&mut parent, n);
            if r != root {
                parent[r as usize] = root;
            }
        }
    }
    let mut has = vec![false; nn];
    let mut is_root = vec![false; nn];
    for n in 0..nn as u32 {
        let r = find(&mut parent, n) as usize;
        is_root[r] = true;
        if constrained[n as usize] {
            has[r] = true;
        }
    }
    let total = is_root.iter().filter(|&&r| r).count();
    let free = (0..nn).filter(|&n| is_root[n] && !has[n]).count();
    (free, total)
}

/// Sistem terakit: model elemen, syarat batas, K, dan prekondisi.
pub(crate) struct Prepared<'a> {
    pub elems: Box<dyn ElementModel + 'a>,
    pub conditions: BoundaryConditions,
    pub assembled: Assembled,
    pub free_dofs: usize,
    pub pre: Box<dyn Preconditioner>,
}

/// Membangun syarat batas, memeriksa tumpuan, merakit K dan prekondisi.
/// `require_support = false` mengizinkan model tanpa tumpuan (frekuensi
/// bebas-bebas).
pub(crate) fn prepare<'a>(
    mesh: MeshRef<'a>,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    extra: &[NodeConstraint],
    require_support: bool,
    cancel: &CancelToken,
) -> Result<Prepared<'a>, SimError> {
    validate_material(material)?;
    let nn = mesh.nodes().len();
    let cell_mm = mesh.cell_mm();
    let elems = mesh.element_model(material)?;
    let conditions = bc::build(mesh.boundary(), material, setup, extra)?;
    check_cancel(cancel)?;

    let free_dofs = conditions.fixed.iter().filter(|&&f| !f).count();
    let mut constrained = vec![false; nn];
    for n in 0..nn {
        constrained[n] =
            conditions.fixed[3 * n] || conditions.fixed[3 * n + 1] || conditions.fixed[3 * n + 2];
    }
    for p in &conditions.penalties {
        constrained[p.node as usize] = true;
    }
    if require_support {
        if !constrained.iter().any(|&c| c) {
            return Err(SimError::Underconstrained {
                free_dofs,
                detail: "the study has no fixtures".into(),
            });
        }
        let (floating, parts) = unconstrained_components(elems.as_ref(), &constrained);
        if floating > 0 {
            return Err(if parts > 1 {
                SimError::MeshTooCoarse {
                    cell_mm,
                    elements: mesh.elements(),
                    detail: format!(
                        "the mesh splits into {parts} disconnected parts and {floating} of them touch no fixture"
                    ),
                }
            } else {
                SimError::Underconstrained {
                    free_dofs,
                    detail: "no fixture touches the body".into(),
                }
            });
        }
    }

    let assembled = assemble_stiffness(elems.as_ref(), &conditions.fixed, &conditions.penalties)?;
    check_cancel(cancel)?;
    // Jacobi blok + koreksi kasar mode tegar; Jacobi blok saja bila model
    // terlalu kecil untuk ruang kasar.
    let pre: Box<dyn Preconditioner> = match TwoLevel::build(
        &assembled.k,
        &conditions.fixed,
        mesh.nodes(),
        mesh.spacing(),
    ) {
        Some(two_level) => Box::new(two_level),
        None => Box::new(BlockJacobi3::new(&assembled.k)),
    };
    check_cancel(cancel)?;
    Ok(Prepared {
        elems,
        conditions,
        assembled,
        free_dofs,
        pre,
    })
}

/// Beban termal: ΔT per node dan koefisien muai.
#[derive(Clone, Copy)]
pub(crate) struct ThermalLoad<'a> {
    pub delta_t: &'a [f64],
    pub alpha: f64,
}

/// Hasil inti statik yang dipakai analisis lain (buckling).
pub(crate) struct StaticOutcome<'a> {
    pub report: SimReport,
    pub prepared: Prepared<'a>,
    /// Tegangan nodal (Voigt).
    pub stress: Vec<[f64; 6]>,
}

/// Studi statik pada mesh apa pun.
pub(crate) fn static_core<'a>(
    mesh: MeshRef<'a>,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    extra: &[NodeConstraint],
    thermal: Option<ThermalLoad<'_>>,
    rel_tol: Option<f64>,
    cancel: &CancelToken,
) -> Result<StaticOutcome<'a>, SimError> {
    let prepared = prepare(mesh, material, setup, extra, true, cancel)?;
    let nn = mesh.nodes().len();
    let elems = prepared.elems.as_ref();
    let conditions = &prepared.conditions;
    let free_dofs = prepared.free_dofs;

    // Gaya luar total: mekanik + ekuivalen termal.
    let mut total_force = conditions.force.clone();
    if let Some(load) = thermal {
        if load.delta_t.len() != nn {
            return Err(SimError::InvalidSetup(
                "temperature field does not match the mesh".into(),
            ));
        }
        let npe = elems.nodes_per_elem();
        let mut dt = vec![0.0; npe];
        let mut f_e = vec![0.0; 3 * npe];
        for e in 0..elems.num_elems() {
            let conn = elems.elem_nodes(e);
            for (slot, &n) in dt.iter_mut().zip(conn) {
                *slot = load.delta_t[n as usize];
            }
            elems.thermal_load(e, load.alpha, &dt, &mut f_e);
            for (l, &n) in conn.iter().enumerate() {
                for a in 0..3 {
                    total_force[3 * n as usize + a] += f_e[3 * l + a];
                }
            }
        }
    }
    let mut rhs = total_force.clone();
    for (b, &fixed) in rhs.iter_mut().zip(&conditions.fixed) {
        if fixed {
            *b = 0.0;
        }
    }
    let mut options = CgOptions::for_size(free_dofs);
    if let Some(tol) = rel_tol {
        options.rel_tol = tol;
    }
    let (u, stats) = solve_pcg(
        &prepared.assembled.k,
        &rhs,
        prepared.pre.as_ref(),
        &options,
        cancel,
    )
    .map_err(|e| match e {
        SimError::Underconstrained { detail, .. } => {
            SimError::Underconstrained { free_dofs, detail }
        }
        other => other,
    })?;
    check_cancel(cancel)?;

    // Post-proses.
    let mut stress = nodal_stress(elems, &u);
    if let Some(load) = thermal {
        // σ = D·(ε − ε_th): kurangi bagian termal pada komponen normal.
        let beta = elems.thermal_modulus() * load.alpha;
        for (s, &dt) in stress.iter_mut().zip(load.delta_t) {
            for r in 0..3 {
                s[r] -= beta * dt;
            }
        }
    }
    let vm: Vec<f64> = stress.iter().map(von_mises).collect();
    let mut max_vm = 0.0;
    let mut max_node = 0usize;
    for (n, &v) in vm.iter().enumerate() {
        if v > max_vm {
            max_vm = v;
            max_node = n;
        }
    }
    let displacement: Vec<[f64; 3]> = u.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    let max_disp = displacement
        .iter()
        .map(|d| dot(*d, *d).sqrt())
        .fold(0.0_f64, f64::max);

    let mut warnings = mesh.warnings().to_vec();
    warnings.extend(conditions.warnings.iter().cloned());
    if stats.iterations == 0 {
        warnings.push("the study has no loads; all results are zero".into());
    }
    let safety_factor = if material.yield_mpa <= 0.0 {
        warnings.push("material has no yield strength; safety factor reported as 0".into());
        0.0
    } else if max_vm * SAFETY_FACTOR_CAP <= material.yield_mpa {
        SAFETY_FACTOR_CAP
    } else {
        material.yield_mpa / max_vm
    };

    // Reaksi: K·u − f_luar pada DOF tereliminasi, −k·(n·u)·n pada penalti.
    let f_int = internal_forces(elems, &u);
    let mut reactions: Vec<Reaction> = setup
        .fixtures
        .iter()
        .map(|f| Reaction {
            fixture_id: f.id.clone(),
            force_n: [0.0; 3],
        })
        .collect();
    for n in 0..nn {
        let owner = conditions.fixed_owner[n];
        if owner == NO_INDEX {
            continue;
        }
        if let Some(r) = reactions.get_mut(owner as usize) {
            for a in 0..3 {
                r.force_n[a] += f_int[3 * n + a] - total_force[3 * n + a];
            }
        }
    }
    for p in &conditions.penalties {
        let d = displacement[p.node as usize];
        // Komponen yang dieliminasi lewat kekangan tambahan tidak memikul penalti.
        let mut dir = p.dir;
        for a in 0..3 {
            if conditions.fixed[3 * p.node as usize + a] {
                dir[a] = 0.0;
            }
        }
        let f = scale(dir, -prepared.assembled.penalty * dot(dir, d));
        if let Some(r) = reactions.get_mut(p.owner as usize) {
            for a in 0..3 {
                r.force_n[a] += f[a];
            }
        }
    }

    let report = SimReport {
        max_von_mises_mpa: max_vm,
        location: mesh.nodes().get(max_node).copied().unwrap_or([0.0; 3]),
        max_displacement_mm: max_disp,
        safety_factor,
        reactions,
        mesh_stats: mesh.stats(),
        solver_stats: SolverStats {
            iterations: stats.iterations,
            residual: stats.residual,
            dofs: free_dofs,
        },
        warnings,
        nodal_field: Some(Field::new(
            mesh.nodes().to_vec(),
            mesh.locator(),
            displacement,
            vm,
        )),
    };
    Ok(StaticOutcome {
        report,
        prepared,
        stress,
    })
}

/// Studi statik linier lengkap dari mesh permukaan. `setup.mesh.kind`
/// memilih mesh hex voxel atau Tet10 (dengan fallback otomatis ke hex).
pub fn run_static(
    surface: &SurfaceMesh,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    cancel: &CancelToken,
) -> Result<SimReport, SimError> {
    validate_material(material)?;
    // Validasi material elemen sebelum pekerjaan berat.
    Hex8::new([1.0; 3], material.young_mpa, material.poisson)?;
    let mesh = build_mesh(
        surface,
        &setup.mesh,
        setup.exact_volume_mm3,
        &setup_faces(setup),
        cancel,
    )?;
    solve_static_on(&mesh, material, setup, &[], cancel)
}

/// Studi statik pada mesh voxel yang sudah ada. `extra` menambah kekangan
/// tingkat node (eliminasi tepat) di luar fixture berbasis face.
pub fn solve_static(
    model: &VoxelModel,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    extra: &[NodeConstraint],
    cancel: &CancelToken,
) -> Result<SimReport, SimError> {
    Ok(static_core(
        MeshRef::Hex(model),
        material,
        setup,
        extra,
        None,
        None,
        cancel,
    )?
    .report)
}

/// Studi statik pada mesh ([`SimMesh`]) yang sudah ada, hex maupun tet.
pub fn solve_static_on(
    mesh: &SimMesh,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    extra: &[NodeConstraint],
    cancel: &CancelToken,
) -> Result<SimReport, SimError> {
    Ok(static_core(mesh.as_ref(), material, setup, extra, None, None, cancel)?.report)
}
