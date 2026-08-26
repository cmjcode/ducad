//! Modul Manajemen Pohon Perakitan (Assembly Tree) & Mate Constraints untuk DuCAD.
//!
//! Menyimpan representasi hierarki perakitan multi-komponen:
//! - Part instances mandiri dengan matriks posisi/rotasi 3D.
//! - Sub-assemblies untuk pengelompokan komponen.
//! - Relasi Mate Constraints 3D (Concentric, Coincident, Distance, Angle).
//! - Pelacakan derajat kebebasan (Degrees of Freedom - DOF).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identifier unik untuk instance part dalam perakitan.
pub type AssemblyInstanceId = u32;

/// Identifier unik untuk kendala perakitan (Mate Constraint).
pub type MateConstraintId = u32;

/// Identifier unik untuk sub-assembly.
pub type SubAssemblyId = u32;

/// Karakteristik geometris entitas target yang di-mate pada suatu part instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MateTargetKind {
    /// Permukaan datar (Planar Face) dengan titik acuan dan vektor normal satuan.
    PlanarFace {
        origin: (f64, f64, f64),
        normal: (f64, f64, f64),
    },
    /// Sumbu permukaan silinder / lubang (Cylinder Axis) dengan titik acuan, arah sumbu satuan, dan radius.
    CylinderAxis {
        origin: (f64, f64, f64),
        direction: (f64, f64, f64),
        radius: f64,
    },
    /// Titik sudut / titik acuan 3D (Point / Vertex).
    Point {
        pos: (f64, f64, f64),
    },
}

impl MateTargetKind {
    pub fn origin(&self) -> (f64, f64, f64) {
        match self {
            MateTargetKind::PlanarFace { origin, .. } => *origin,
            MateTargetKind::CylinderAxis { origin, .. } => *origin,
            MateTargetKind::Point { pos } => *pos,
        }
    }
}

/// Target mate spesifik: pasangan ID instance dan data geometri target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MateTarget {
    pub instance_id: AssemblyInstanceId,
    pub kind: MateTargetKind,
}

/// Jenis hubungan kendala perakitan 3D (Mate Kind).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MateKind {
    /// Menyelaraskan sumbu silinder poros dengan sumbu lubang silinder (kolinear).
    Concentric {
        /// Apakah rotasi pada sumbu silinder dikunci (Lock Rotation).
        lock_rotation: bool,
        /// Apakah arah sumbu sejajar (true) atau berlawanan arah (false).
        aligned: bool,
    },
    /// Menempelkan dua permukaan planar datar saling berhimpit (coplanar).
    Coincident {
        /// Apakah vektor normal kedua permukaan saling berhadapan (kontak muka-ke-muka, true)
        /// atau searah (false).
        opposite_normal: bool,
    },
    /// Menetapkan jarak terukur offset $d$ mm antara dua permukaan/titik acuan.
    Distance {
        offset: f64,
        opposite_normal: bool,
    },
    /// Menetapkan sudut rotasi engsel $\theta^\circ$ antara dua bidang atau garis acuan.
    Angle {
        angle_deg: f64,
        opposite_normal: bool,
    },
}

impl MateKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            MateKind::Concentric { .. } => "Concentric",
            MateKind::Coincident { .. } => "Coincident",
            MateKind::Distance { .. } => "Distance",
            MateKind::Angle { .. } => "Angle",
        }
    }
}

/// Status evaluasi kendala mate perakitan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MateStatus {
    /// Mate terpenuhi dengan sempurna oleh posisi geometri saat ini.
    Satisfied,
    /// Komponen masih memiliki derajat kebebasan gerak (Under-constrained).
    UnderConstrained,
    /// Terjadi benturan / konflik geometris antar mate (Over-constrained / Conflicted).
    Conflicted(String),
    /// Mate dinonaktifkan sementara oleh pengguna.
    Suppressed,
}

impl Default for MateStatus {
    fn default() -> Self {
        Self::Satisfied
    }
}

/// Satu relasi kendala perakitan 3D (Mate Constraint) antar dua entitas part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MateConstraint {
    pub id: MateConstraintId,
    pub name: String,
    pub kind: MateKind,
    pub target_a: MateTarget,
    pub target_b: MateTarget,
    pub status: MateStatus,
    pub suppressed: bool,
}

/// Derajat kebebasan (Degrees of Freedom - DOF) sebuah komponen dalam perakitan 3D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegreesOfFreedom {
    /// Derajat kebebasan translasi (0 - 3: X, Y, Z).
    pub translation_dof: u8,
    /// Derajat kebebasan rotasi (0 - 3: Pitch, Yaw, Roll).
    pub rotation_dof: u8,
}

impl DegreesOfFreedom {
    pub fn free() -> Self {
        Self {
            translation_dof: 3,
            rotation_dof: 3,
        }
    }

    pub fn fixed() -> Self {
        Self {
            translation_dof: 0,
            rotation_dof: 0,
        }
    }

    pub fn total_dof(&self) -> u8 {
        self.translation_dof + self.rotation_dof
    }

    pub fn is_fully_constrained(&self) -> bool {
        self.total_dof() == 0
    }
}

/// Satu instance part dalam perakitan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyInstance {
    pub id: AssemblyInstanceId,
    pub name: String,
    /// ID Body solid asal di ModelDoc.
    pub body_id_raw: u64,
    /// Apakah posisi komponen ini dikunci mati sebagai jangkar perakitan (Grounded).
    pub is_grounded: bool,
    /// Vektor translasi posisi 3D (X, Y, Z) dalam koordinat ruang perakitan.
    pub translation: (f64, f64, f64),
    /// Orientasi rotasi 3D dalam format Quaternion (X, Y, Z, W).
    pub rotation_quat: (f64, f64, f64, f64),
    /// Visibilitas instance dalam viewport.
    pub visible: bool,
    /// ID Sub-Assembly induk jika berada dalam kelompok sub-assembly.
    pub parent_sub_assembly: Option<SubAssemblyId>,
}

impl AssemblyInstance {
    pub fn new(id: AssemblyInstanceId, name: impl Into<String>, body_id_raw: u64) -> Self {
        Self {
            id,
            name: name.into(),
            body_id_raw,
            is_grounded: false,
            translation: (0.0, 0.0, 0.0),
            rotation_quat: (0.0, 0.0, 0.0, 1.0), // Identity quaternion
            visible: true,
            parent_sub_assembly: None,
        }
    }
}

/// Node kelompok Sub-Assembly dalam hierarki perakitan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubAssembly {
    pub id: SubAssemblyId,
    pub name: String,
    pub expanded: bool,
    pub parent_sub_assembly: Option<SubAssemblyId>,
}

/// Struktur data lengkap Pohon Hierarki Perakitan (Assembly Tree).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyTree {
    pub instances: HashMap<AssemblyInstanceId, AssemblyInstance>,
    pub sub_assemblies: HashMap<SubAssemblyId, SubAssembly>,
    pub mates: HashMap<MateConstraintId, MateConstraint>,
    pub next_instance_id: AssemblyInstanceId,
    pub next_mate_id: MateConstraintId,
    pub next_sub_id: SubAssemblyId,
}

impl Default for AssemblyTree {
    fn default() -> Self {
        Self {
            instances: HashMap::new(),
            sub_assemblies: HashMap::new(),
            mates: HashMap::new(),
            next_instance_id: 1,
            next_mate_id: 1,
            next_sub_id: 1,
        }
    }
}

impl AssemblyTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tambahkan instance part baru ke perakitan.
    /// Jika ini instance pertama dalam perakitan, otomatis jadikan `is_grounded = true`.
    pub fn add_instance(&mut self, name: impl Into<String>, body_id_raw: u64) -> AssemblyInstanceId {
        let id = self.next_instance_id;
        self.next_instance_id += 1;
        let is_first = self.instances.is_empty();
        let mut instance = AssemblyInstance::new(id, name, body_id_raw);
        if is_first {
            instance.is_grounded = true;
        }
        self.instances.insert(id, instance);
        id
    }

    /// Hapus instance part dan semua mate constraints yang terhubung dengannya.
    pub fn remove_instance(&mut self, id: AssemblyInstanceId) {
        self.instances.remove(&id);
        // Hapus mate yang mereferensikan instance ini
        self.mates
            .retain(|_, m| m.target_a.instance_id != id && m.target_b.instance_id != id);
    }

    /// Set status grounded (terkunci/tidak) untuk suatu instance part.
    pub fn set_grounded(&mut self, id: AssemblyInstanceId, grounded: bool) {
        if let Some(inst) = self.instances.get_mut(&id) {
            inst.is_grounded = grounded;
        }
    }

    /// Tambahkan Sub-Assembly baru untuk pengelompokan hierarkis.
    pub fn add_sub_assembly(
        &mut self,
        name: impl Into<String>,
        parent: Option<SubAssemblyId>,
    ) -> SubAssemblyId {
        let id = self.next_sub_id;
        self.next_sub_id += 1;
        self.sub_assemblies.insert(
            id,
            SubAssembly {
                id,
                name: name.into(),
                expanded: true,
                parent_sub_assembly: parent,
            },
        );
        id
    }

    /// Hapus Sub-Assembly dari pohon perakitan.
    pub fn remove_sub_assembly(&mut self, id: SubAssemblyId) -> Option<SubAssembly> {
        // Lepas relasi parent untuk anak-anaknya
        for inst in self.instances.values_mut() {
            if inst.parent_sub_assembly == Some(id) {
                inst.parent_sub_assembly = None;
            }
        }
        for sub in self.sub_assemblies.values_mut() {
            if sub.parent_sub_assembly == Some(id) {
                sub.parent_sub_assembly = None;
            }
        }
        self.sub_assemblies.remove(&id)
    }

    /// Pindahkan instance ke dalam sub-assembly tertentu (atau keluar jika `None`).
    pub fn set_instance_parent(
        &mut self,
        instance_id: AssemblyInstanceId,
        parent: Option<SubAssemblyId>,
    ) {
        if let Some(inst) = self.instances.get_mut(&instance_id) {
            inst.parent_sub_assembly = parent;
        }
    }

    /// Tambahkan Mate Constraint 3D baru antara dua target.
    pub fn add_mate(
        &mut self,
        name: impl Into<String>,
        kind: MateKind,
        target_a: MateTarget,
        target_b: MateTarget,
    ) -> MateConstraintId {
        let id = self.next_mate_id;
        self.next_mate_id += 1;
        let constraint = MateConstraint {
            id,
            name: name.into(),
            kind,
            target_a,
            target_b,
            status: MateStatus::Satisfied,
            suppressed: false,
        };
        self.mates.insert(id, constraint);
        id
    }

    /// Hapus Mate Constraint.
    pub fn remove_mate(&mut self, id: MateConstraintId) {
        self.mates.remove(&id);
    }

    /// Aktifkan / Nonaktifkan (Suppress) Mate Constraint.
    pub fn toggle_suppress_mate(&mut self, id: MateConstraintId) {
        if let Some(mate) = self.mates.get_mut(&id) {
            mate.suppressed = !mate.suppressed;
            if mate.suppressed {
                mate.status = MateStatus::Suppressed;
            } else {
                mate.status = MateStatus::Satisfied;
            }
        }
    }

    /// Hitung estimasi Derajat Kebebasan (DOF) untuk suatu instance berdasarkan mate aktif.
    pub fn compute_instance_dof(&self, instance_id: AssemblyInstanceId) -> DegreesOfFreedom {
        let Some(inst) = self.instances.get(&instance_id) else {
            return DegreesOfFreedom::free();
        };
        if inst.is_grounded {
            return DegreesOfFreedom::fixed();
        }

        let mut trans_dof = 3i32;
        let mut rot_dof = 3i32;

        for mate in self.mates.values() {
            if mate.suppressed {
                continue;
            }
            if mate.target_a.instance_id == instance_id || mate.target_b.instance_id == instance_id
            {
                match &mate.kind {
                    MateKind::Concentric { lock_rotation, .. } => {
                        // Concentric mengunci 2 translasi tegak lurus sumbu dan 2 rotasi miring
                        trans_dof -= 2;
                        rot_dof -= 2;
                        if *lock_rotation {
                            rot_dof -= 1;
                        }
                    }
                    MateKind::Coincident { .. } | MateKind::Distance { .. } => {
                        // Coincident/Distance bidang mengunci 1 translasi normal dan 2 rotasi miring
                        trans_dof -= 1;
                        rot_dof -= 2;
                    }
                    MateKind::Angle { .. } => {
                        // Angle mengunci 1 rotasi
                        rot_dof -= 1;
                    }
                }
            }
        }

        DegreesOfFreedom {
            translation_dof: trans_dof.clamp(0, 3) as u8,
            rotation_dof: rot_dof.clamp(0, 3) as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assembly_tree_instance_management() {
        let mut tree = AssemblyTree::new();
        let inst1 = tree.add_instance("Base Plate", 1001);
        let inst2 = tree.add_instance("Shaft Pin", 1002);

        assert_eq!(tree.instances.len(), 2);
        assert!(tree.instances[&inst1].is_grounded); // Instance pertama otomatis grounded
        assert!(!tree.instances[&inst2].is_grounded);

        let dof1 = tree.compute_instance_dof(inst1);
        assert_eq!(dof1.total_dof(), 0); // Grounded = 0 DOF

        let dof2 = tree.compute_instance_dof(inst2);
        assert_eq!(dof2.total_dof(), 6); // Free = 6 DOF
    }

    #[test]
    fn test_mate_constraint_dof_calculation() {
        let mut tree = AssemblyTree::new();
        let inst1 = tree.add_instance("Base Block", 1);
        let inst2 = tree.add_instance("Cylinder Pin", 2);

        // Tambahkan Concentric Mate
        let concentric_id = tree.add_mate(
            "Concentric1",
            MateKind::Concentric {
                lock_rotation: false,
                aligned: true,
            },
            MateTarget {
                instance_id: inst1,
                kind: MateTargetKind::CylinderAxis {
                    origin: (0.0, 0.0, 0.0),
                    direction: (0.0, 0.0, 1.0),
                    radius: 5.0,
                },
            },
            MateTarget {
                instance_id: inst2,
                kind: MateTargetKind::CylinderAxis {
                    origin: (10.0, 20.0, 0.0),
                    direction: (0.0, 0.0, 1.0),
                    radius: 5.0,
                },
            },
        );

        let dof_concentric = tree.compute_instance_dof(inst2);
        // Concentric: menyisakan 1 translasi sepanjang sumbu dan 1 rotasi mengelilingi sumbu (Total = 2 DOF)
        assert_eq!(dof_concentric.translation_dof, 1);
        assert_eq!(dof_concentric.rotation_dof, 1);
        assert_eq!(dof_concentric.total_dof(), 2);

        // Tambahkan Coincident Mate pada shoulder face
        tree.add_mate(
            "Coincident1",
            MateKind::Coincident {
                opposite_normal: true,
            },
            MateTarget {
                instance_id: inst1,
                kind: MateTargetKind::PlanarFace {
                    origin: (0.0, 0.0, 20.0),
                    normal: (0.0, 0.0, 1.0),
                },
            },
            MateTarget {
                instance_id: inst2,
                kind: MateTargetKind::PlanarFace {
                    origin: (0.0, 0.0, 0.0),
                    normal: (0.0, 0.0, -1.0),
                },
            },
        );

        let dof_final = tree.compute_instance_dof(inst2);
        // Menghilangkan translasi z, menyisakan rotasi bebas jika lock_rotation = false
        assert_eq!(dof_final.translation_dof, 0);

        // Test suppress
        tree.toggle_suppress_mate(concentric_id);
        assert_eq!(tree.mates[&concentric_id].status, MateStatus::Suppressed);
    }
}
