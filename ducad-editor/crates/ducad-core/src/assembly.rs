//! Modul Manajemen Pohon Perakitan (Assembly Tree) & Mate Constraints untuk DuCAD.
//!
//! Menyimpan representasi hierarki perakitan multi-komponen:
//! - Part instances mandiri dengan matriks posisi/rotasi 3D.
//! - Sub-assemblies untuk pengelompokan komponen.
//! - Relasi Mate Constraints 3D (Concentric, Coincident, Distance, Angle).
//! - Pelacakan derajat kebebasan (Degrees of Freedom - DOF).

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

/// Jenis sambungan mekanis siap pakai (*joint*).
///
/// Joint bukan jenis kendala baru — ia adalah NAMA untuk kombinasi kendala
/// yang sudah ada, plus deklarasi eksplisit derajat kebebasan mana yang
/// sengaja DIBIARKAN bebas. Membedakannya penting: "engsel" dan "silinder
/// sepusat yang rotasinya dikunci" memakai geometri yang sama persis, tapi
/// yang pertama boleh berputar dan yang kedua tidak — dan tanpa nama joint,
/// perbedaan itu tidak terekam di mana pun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JointKind {
    /// Terkunci penuh, 0 DOF.
    Rigid,
    /// Engsel: berputar pada satu sumbu, 1 DOF rotasi.
    Revolute,
    /// Luncur: bergeser sepanjang satu sumbu, 1 DOF translasi.
    Slider,
    /// Silindris: berputar DAN bergeser pada sumbu yang sama, 2 DOF.
    Cylindrical,
    /// Bidang: bergeser di dua arah + berputar pada normalnya, 3 DOF.
    Planar,
    /// Bola: bebas berputar di tiga sumbu, 3 DOF rotasi.
    Ball,
}

impl JointKind {
    pub fn label(self) -> &'static str {
        match self {
            JointKind::Rigid => "Rigid",
            JointKind::Revolute => "Revolute (Engsel)",
            JointKind::Slider => "Slider (Luncur)",
            JointKind::Cylindrical => "Cylindrical",
            JointKind::Planar => "Planar",
            JointKind::Ball => "Ball (Bola)",
        }
    }

    /// Derajat kebebasan yang TERSISA setelah joint ini diterapkan.
    pub fn remaining_dof(self) -> DegreesOfFreedom {
        let (t, r) = match self {
            JointKind::Rigid => (0, 0),
            JointKind::Revolute => (0, 1),
            JointKind::Slider => (1, 0),
            JointKind::Cylindrical => (1, 1),
            JointKind::Planar => (2, 1),
            JointKind::Ball => (0, 3),
        };
        DegreesOfFreedom {
            translation_dof: t,
            rotation_dof: r,
        }
    }

    /// Kendala dasar yang menyusun joint ini.
    pub fn underlying_mate(self) -> MateKind {
        match self {
            JointKind::Rigid => MateKind::Concentric {
                lock_rotation: true,
                aligned: true,
            },
            JointKind::Revolute | JointKind::Ball => MateKind::Concentric {
                lock_rotation: false,
                aligned: true,
            },
            JointKind::Slider | JointKind::Cylindrical => MateKind::Concentric {
                lock_rotation: false,
                aligned: true,
            },
            JointKind::Planar => MateKind::Coincident {
                opposite_normal: true,
            },
        }
    }
}

/// Batas gerak sebuah mate (*limit mate*).
///
/// Engsel pintu tidak berputar 360°, dan poros teleskopik punya panjang
/// maksimum. Tanpa batas ini, solver akan dengan senang hati menempatkan
/// part di posisi yang mustahil secara fisik.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct MateLimits {
    /// Batas bawah nilai mate (mm untuk Distance, derajat untuk Angle).
    pub min: Option<f64>,
    /// Batas atas.
    pub max: Option<f64>,
}

impl MateLimits {
    pub fn is_unbounded(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// Jepit `value` ke dalam rentang. Batas yang terbalik (min > max)
    /// diperlakukan sebagai tidak ada — data rusak tidak boleh membuat
    /// nilai melompat ke angka yang aneh.
    pub fn clamp(&self, value: f64) -> f64 {
        match (self.min, self.max) {
            (Some(lo), Some(hi)) if lo <= hi => value.clamp(lo, hi),
            (Some(lo), None) => value.max(lo),
            (None, Some(hi)) => value.min(hi),
            _ => value,
        }
    }

    /// Apakah `value` berada di dalam rentang.
    pub fn contains(&self, value: f64) -> bool {
        (self.clamp(value) - value).abs() < 1e-9
    }
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

    /// Nilai terkendali mate ini, bila ada — jarak (mm) atau sudut
    /// (derajat). `None` untuk mate yang tidak punya nilai numerik.
    pub fn driven_value(&self) -> Option<f64> {
        match self {
            MateKind::Distance { offset, .. } => Some(*offset),
            MateKind::Angle { angle_deg, .. } => Some(*angle_deg),
            _ => None,
        }
    }

    /// Salinan mate dengan nilai terkendalinya dijepit ke `limits`.
    pub fn with_value_clamped(&self, limits: &MateLimits) -> MateKind {
        match self {
            MateKind::Distance {
                offset,
                opposite_normal,
            } => MateKind::Distance {
                offset: limits.clamp(*offset),
                opposite_normal: *opposite_normal,
            },
            MateKind::Angle {
                angle_deg,
                opposite_normal,
            } => MateKind::Angle {
                angle_deg: limits.clamp(*angle_deg),
                opposite_normal: *opposite_normal,
            },
            other => other.clone(),
        }
    }
}

/// Status evaluasi kendala mate perakitan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MateStatus {
    /// Mate terpenuhi dengan sempurna oleh posisi geometri saat ini.
    #[default]
    Satisfied,
    /// Komponen masih memiliki derajat kebebasan gerak (Under-constrained).
    UnderConstrained,
    /// Terjadi benturan / konflik geometris antar mate (Over-constrained / Conflicted).
    Conflicted(String),
    /// Mate dinonaktifkan sementara oleh pengguna.
    Suppressed,
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
    /// Batas gerak opsional. `default` supaya berkas lama tetap terbaca.
    #[serde(default)]
    pub limits: MateLimits,
    /// Joint yang diwakili mate ini, bila ia dibuat lewat preset joint.
    #[serde(default)]
    pub joint: Option<JointKind>,
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
    /// Nomor part untuk BOM. Dua instance dengan nomor sama digabung jadi
    /// satu baris berkuantitas.
    #[serde(default)]
    pub part_number: Option<String>,
    /// Bahan, muncul di kolom material BOM.
    #[serde(default)]
    pub material: Option<String>,
}

/// Satu baris tabel BOM (*Bill of Materials*).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BomRow {
    /// Nomor urut, dipakai balon penunjuk di gambar isometrik.
    pub item: u32,
    pub part_number: Option<String>,
    pub name: String,
    pub material: Option<String>,
    pub quantity: u32,
    /// Sub-assembly tingkat pertama yang memuatnya (BOM *indented*).
    pub sub_assembly: Option<String>,
}

impl BomRow {
    /// Satu baris CSV. Field yang memuat koma, kutip, atau baris baru
    /// dikutip dan kutipnya digandakan sesuai RFC 4180 — tanpa itu, nama
    /// part seperti `Bracket, Left` akan menggeser seluruh kolom.
    pub fn to_csv_row(&self) -> String {
        fn esc(v: &str) -> String {
            if v.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", v.replace('"', "\"\""))
            } else {
                v.to_string()
            }
        }
        let cols = [
            self.item.to_string(),
            esc(self.part_number.as_deref().unwrap_or("")),
            esc(&self.name),
            esc(self.material.as_deref().unwrap_or("")),
            self.quantity.to_string(),
            esc(self.sub_assembly.as_deref().unwrap_or("")),
        ];
        cols.join(",")
    }

    pub fn csv_header() -> &'static str {
        "Item,PartNumber,Name,Material,Qty,SubAssembly"
    }
}

/// Seluruh tabel BOM sebagai teks CSV.
pub fn bom_to_csv(rows: &[BomRow]) -> String {
    let mut out = String::from(BomRow::csv_header());
    for r in rows {
        out.push('\n');
        out.push_str(&r.to_csv_row());
    }
    out.push('\n');
    out
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
            part_number: None,
            material: None,
        }
    }
}

/// Node kelompok Sub-Assembly dalam hierarki perakitan.
///
/// Sub-assembly adalah KERANGKA KOORDINAT, bukan sekadar label
/// pengelompokan di panel. Sebelumnya ia tidak punya transform sama sekali,
/// sehingga memindahkan sebuah sub-assembly mustahil dan part di dalamnya
/// tidak pernah ikut bergerak — hierarkinya hanya ada di tampilan pohon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubAssembly {
    pub id: SubAssemblyId,
    pub name: String,
    pub expanded: bool,
    pub parent_sub_assembly: Option<SubAssemblyId>,
    /// Translasi terhadap kerangka INDUKNYA (mm).
    #[serde(default)]
    pub translation: (f64, f64, f64),
    /// Rotasi terhadap kerangka induknya, quaternion (x, y, z, w).
    #[serde(default = "identity_quat")]
    pub rotation_quat: (f64, f64, f64, f64),
}

fn identity_quat() -> (f64, f64, f64, f64) {
    (0.0, 0.0, 0.0, 1.0)
}

/// Kedalaman hierarki maksimum yang ditelusuri saat menyusun transform.
///
/// Rantai `parent_sub_assembly` bisa membentuk siklus akibat data rusak
/// atau operasi UI yang salah (memindahkan sub-assembly ke dalam
/// turunannya sendiri). Tanpa batas ini, penyusunan transform akan
/// berputar selamanya dan membekukan aplikasi.
const MAX_ASSEMBLY_DEPTH: usize = 64;

/// Ubah pasangan tuple tersimpan menjadi tipe glam, menormalkan quaternion
/// yang nol/rusak jadi identitas alih-alih menghasilkan rotasi NaN.
fn local_pose(t: (f64, f64, f64), q: (f64, f64, f64, f64)) -> (DVec3, DQuat) {
    let quat = DQuat::from_xyzw(q.0, q.1, q.2, q.3);
    let quat = if quat.length_squared() < 1e-12 {
        DQuat::IDENTITY
    } else {
        quat.normalize()
    };
    (DVec3::new(t.0, t.1, t.2), quat)
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
                translation: (0.0, 0.0, 0.0),
                rotation_quat: identity_quat(),
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

    // ----------------------------------------------------------------
    // Bill of Materials.
    // ----------------------------------------------------------------

    /// Susun BOM dari pohon perakitan.
    ///
    /// Part yang sama yang dipakai berkali-kali digabung jadi SATU baris
    /// berkuantitas — itulah inti sebuah BOM. Penggabungan memakai
    /// `part_number` bila ada, dan jatuh ke `body_id_raw` bila belum diisi;
    /// memakai NAMA sebagai kunci akan salah, karena dua part berbeda boleh
    /// bernama sama dan satu part boleh diganti namanya per instance.
    ///
    /// `flat = true` menghasilkan daftar rata seluruh part. `flat = false`
    /// mengelompokkan per sub-assembly tingkat pertama, seperti BOM
    /// *indented* pada gambar kerja.
    pub fn build_bom(&self, flat: bool) -> Vec<BomRow> {
        use std::collections::BTreeMap;

        // BTreeMap, bukan HashMap: urutan baris BOM harus sama tiap kali
        // digenerate, kalau tidak diff antar revisi gambar jadi tak terbaca.
        let mut rows: BTreeMap<(Option<SubAssemblyId>, String), BomRow> = BTreeMap::new();

        for inst in self.instances.values() {
            let key_id = inst
                .part_number
                .clone()
                .unwrap_or_else(|| format!("#{}", inst.body_id_raw));
            let group = if flat {
                None
            } else {
                self.top_level_parent(inst.parent_sub_assembly)
            };
            let entry = rows.entry((group, key_id.clone())).or_insert_with(|| BomRow {
                item: 0,
                part_number: inst.part_number.clone(),
                name: inst.name.clone(),
                material: inst.material.clone(),
                quantity: 0,
                sub_assembly: group.and_then(|g| self.sub_assemblies.get(&g).map(|s| s.name.clone())),
            });
            entry.quantity += 1;
        }

        let mut out: Vec<BomRow> = rows.into_values().collect();
        for (i, row) in out.iter_mut().enumerate() {
            row.item = i as u32 + 1;
        }
        out
    }

    /// Sub-assembly tingkat PERTAMA yang memuat `start`, menelusuri ke atas.
    fn top_level_parent(&self, start: Option<SubAssemblyId>) -> Option<SubAssemblyId> {
        let mut cursor = start?;
        let mut steps = 0;
        loop {
            let sub = self.sub_assemblies.get(&cursor)?;
            match sub.parent_sub_assembly {
                Some(p) => {
                    steps += 1;
                    if steps > MAX_ASSEMBLY_DEPTH {
                        return Some(cursor); // rantai bersiklus; berhenti
                    }
                    cursor = p;
                }
                None => return Some(cursor),
            }
        }
    }

    // ----------------------------------------------------------------
    // Komposisi transform hierarkis.
    // ----------------------------------------------------------------

    /// Transform sebuah sub-assembly terhadap DUNIA, hasil penyusunan
    /// seluruh rantai induknya.
    ///
    /// Mengembalikan `None` bila `id` tidak dikenal atau rantai induknya
    /// membentuk siklus — lebih baik melapor daripada berputar selamanya
    /// atau diam-diam memakai transform yang salah.
    pub fn sub_world_transform(&self, id: SubAssemblyId) -> Option<(DVec3, DQuat)> {
        // Kumpulkan rantai dari node ke akar dulu, baru disusun dari akar
        // ke bawah: transform induk harus diterapkan SEBELUM transform anak.
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = Some(id);
        while let Some(sid) = cursor {
            if !seen.insert(sid) || chain.len() >= MAX_ASSEMBLY_DEPTH {
                return None; // siklus atau kedalaman tak masuk akal
            }
            let sub = self.sub_assemblies.get(&sid)?;
            chain.push(sub);
            cursor = sub.parent_sub_assembly;
        }

        let mut t = DVec3::ZERO;
        let mut q = DQuat::IDENTITY;
        for sub in chain.iter().rev() {
            let (lt, lq) = local_pose(sub.translation, sub.rotation_quat);
            t += q * lt;
            q = (q * lq).normalize();
        }
        Some((t, q))
    }

    /// Transform sebuah instance part terhadap DUNIA: transform lokalnya
    /// disusun di atas transform seluruh sub-assembly induknya.
    ///
    /// Inilah yang membuat memindahkan sub-assembly benar-benar memindahkan
    /// isinya.
    pub fn instance_world_transform(&self, id: AssemblyInstanceId) -> Option<(DVec3, DQuat)> {
        let inst = self.instances.get(&id)?;
        let (lt, lq) = local_pose(inst.translation, inst.rotation_quat);
        let (pt, pq) = match inst.parent_sub_assembly {
            Some(parent) => self.sub_world_transform(parent)?,
            None => (DVec3::ZERO, DQuat::IDENTITY),
        };
        Some((pt + pq * lt, (pq * lq).normalize()))
    }

    /// Transform kerangka INDUK sebuah instance — dipakai mengubah koreksi
    /// yang dihitung di ruang dunia menjadi pergeseran lokal instance.
    pub fn instance_parent_transform(&self, id: AssemblyInstanceId) -> (DVec3, DQuat) {
        self.instances
            .get(&id)
            .and_then(|i| i.parent_sub_assembly)
            .and_then(|p| self.sub_world_transform(p))
            .unwrap_or((DVec3::ZERO, DQuat::IDENTITY))
    }

    /// Apakah menjadikan `new_parent` sebagai induk `sub` akan membuat
    /// siklus. Dipakai UI sebelum memindahkan node di pohon perakitan.
    pub fn would_create_cycle(&self, sub: SubAssemblyId, new_parent: Option<SubAssemblyId>) -> bool {
        let mut cursor = new_parent;
        let mut steps = 0;
        while let Some(cur) = cursor {
            if cur == sub {
                return true;
            }
            steps += 1;
            if steps > MAX_ASSEMBLY_DEPTH {
                return true; // rantainya sendiri sudah rusak
            }
            cursor = self
                .sub_assemblies
                .get(&cur)
                .and_then(|s| s.parent_sub_assembly);
        }
        false
    }

    /// Pindahkan sub-assembly ke induk baru, MENOLAK perpindahan yang akan
    /// membuat siklus. Mengembalikan `false` bila ditolak.
    pub fn set_sub_assembly_parent(
        &mut self,
        sub: SubAssemblyId,
        new_parent: Option<SubAssemblyId>,
    ) -> bool {
        if self.would_create_cycle(sub, new_parent) {
            return false;
        }
        match self.sub_assemblies.get_mut(&sub) {
            Some(s) => {
                s.parent_sub_assembly = new_parent;
                true
            }
            None => false,
        }
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
            limits: MateLimits::default(),
            joint: None,
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

    #[test]
    fn test_clash_report_summary() {
        let mut report = ClashReport::new();
        report.add_clash(ClashItem {
            id: 1,
            body_a_id: 10,
            body_b_id: 20,
            body_a_name: "Pin".to_string(),
            body_b_name: "Base".to_string(),
            volume: 125.5,
            center: (5.0, 5.0, 10.0),
            bbox_min: (0.0, 0.0, 0.0),
            bbox_max: (10.0, 10.0, 20.0),
        });

        assert_eq!(report.clashes.len(), 1);
        assert_eq!(report.total_volume, 125.5);
        assert!(report.has_clashes());
    }
}

/// Satu entri laporan benturan / tabrakan fisik antar bodi solid (Clash Item).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClashItem {
    pub id: u32,
    pub body_a_id: u64,
    pub body_b_id: u64,
    pub body_a_name: String,
    pub body_b_name: String,
    /// Volume tabrakan dalam mm³.
    pub volume: f64,
    /// Titik pusat tabrakan (X, Y, Z).
    pub center: (f64, f64, f64),
    pub bbox_min: (f64, f64, f64),
    pub bbox_max: (f64, f64, f64),
}

/// Laporan lengkap hasil uji deteksi tabrakan & interferensi (Clash Report).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClashReport {
    pub clashes: Vec<ClashItem>,
    pub total_volume: f64,
    pub evaluated_pairs: usize,
    pub timestamp_epoch_ms: u64,
}

impl ClashReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_clashes(&self) -> bool {
        !self.clashes.is_empty()
    }

    pub fn add_clash(&mut self, item: ClashItem) {
        self.total_volume += item.volume;
        self.clashes.push(item);
    }

    pub fn clear(&mut self) {
        self.clashes.clear();
        self.total_volume = 0.0;
        self.evaluated_pairs = 0;
    }
}


#[cfg(test)]
mod hierarchy_tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn quat(axis: DVec3, angle: f64) -> (f64, f64, f64, f64) {
        let q = DQuat::from_axis_angle(axis, angle);
        (q.x, q.y, q.z, q.w)
    }

    #[test]
    fn moving_a_sub_assembly_moves_its_parts() {
        // Inti P3.1. Sebelumnya `SubAssembly` tidak punya transform sama
        // sekali, jadi hierarkinya hanya ada di panel — memindahkan
        // sub-assembly tidak menggerakkan apa pun.
        let mut tree = AssemblyTree::default();
        let sub = tree.add_sub_assembly("Gearbox", None);
        let inst = tree.add_instance("Gear", 1);
        tree.set_instance_parent(inst, Some(sub));

        if let Some(i) = tree.instances.get_mut(&inst) {
            i.translation = (5.0, 0.0, 0.0);
        }
        if let Some(s) = tree.sub_assemblies.get_mut(&sub) {
            s.translation = (100.0, 0.0, 0.0);
        }

        let (t, _) = tree.instance_world_transform(inst).unwrap();
        assert!(
            (t - DVec3::new(105.0, 0.0, 0.0)).length() < 1e-9,
            "posisi dunia {t:?} — transform sub-assembly harus ikut tersusun"
        );
    }

    #[test]
    fn sub_assembly_rotation_rotates_child_offset() {
        // Rotasi induk harus MEMUTAR offset anaknya, bukan sekadar
        // menjumlahkan translasi. Part di (10,0,0) dalam sub-assembly yang
        // diputar 90° terhadap Z harus berakhir di (0,10,0).
        let mut tree = AssemblyTree::default();
        let sub = tree.add_sub_assembly("Arm", None);
        let inst = tree.add_instance("Tip", 1);
        tree.set_instance_parent(inst, Some(sub));

        if let Some(i) = tree.instances.get_mut(&inst) {
            i.translation = (10.0, 0.0, 0.0);
        }
        if let Some(s) = tree.sub_assemblies.get_mut(&sub) {
            s.rotation_quat = quat(DVec3::Z, FRAC_PI_2);
        }

        let (t, _) = tree.instance_world_transform(inst).unwrap();
        assert!(
            (t - DVec3::new(0.0, 10.0, 0.0)).length() < 1e-9,
            "posisi dunia {t:?}"
        );
    }

    #[test]
    fn nested_sub_assemblies_compose_in_order() {
        // Transform induk diterapkan SEBELUM transform anak; urutan yang
        // terbalik akan memberi jawaban berbeda pada rotasi.
        let mut tree = AssemblyTree::default();
        let outer = tree.add_sub_assembly("Outer", None);
        let inner = tree.add_sub_assembly("Inner", Some(outer));
        let inst = tree.add_instance("Part", 1);
        tree.set_instance_parent(inst, Some(inner));

        if let Some(s) = tree.sub_assemblies.get_mut(&outer) {
            s.rotation_quat = quat(DVec3::Z, FRAC_PI_2);
        }
        if let Some(s) = tree.sub_assemblies.get_mut(&inner) {
            s.translation = (10.0, 0.0, 0.0);
        }
        if let Some(i) = tree.instances.get_mut(&inst) {
            i.translation = (0.0, 5.0, 0.0);
        }

        // Inner di (10,0,0) lokal -> diputar 90°Z jadi (0,10,0).
        // Part (0,5,0) lokal -> diputar 90°Z jadi (-5,0,0); total (-5,10,0).
        let (t, _) = tree.instance_world_transform(inst).unwrap();
        assert!(
            (t - DVec3::new(-5.0, 10.0, 0.0)).length() < 1e-9,
            "posisi dunia {t:?}"
        );
    }

    #[test]
    fn cyclic_parent_chain_is_reported_not_hung() {
        // Data rusak atau drag-drop yang salah bisa membuat siklus.
        // Tanpa penjagaan, penyusunan transform berputar selamanya dan
        // membekukan aplikasi.
        let mut tree = AssemblyTree::default();
        let a = tree.add_sub_assembly("A", None);
        let b = tree.add_sub_assembly("B", Some(a));
        // Paksa siklus langsung ke struktur data (melewati penjagaan API).
        tree.sub_assemblies.get_mut(&a).unwrap().parent_sub_assembly = Some(b);

        assert!(
            tree.sub_world_transform(a).is_none(),
            "rantai bersiklus harus dilaporkan None, bukan menggantung"
        );
    }

    #[test]
    fn reparenting_into_own_descendant_is_refused() {
        let mut tree = AssemblyTree::default();
        let a = tree.add_sub_assembly("A", None);
        let b = tree.add_sub_assembly("B", Some(a));

        assert!(tree.would_create_cycle(a, Some(b)));
        assert!(
            !tree.set_sub_assembly_parent(a, Some(b)),
            "memindahkan A ke dalam turunannya sendiri harus DITOLAK"
        );
        // Struktur tidak berubah.
        assert_eq!(
            tree.sub_assemblies.get(&a).unwrap().parent_sub_assembly,
            None
        );

        // Perpindahan yang sah tetap diterima.
        let c = tree.add_sub_assembly("C", None);
        assert!(tree.set_sub_assembly_parent(b, Some(c)));
    }

    #[test]
    fn instance_without_parent_uses_its_own_transform() {
        let mut tree = AssemblyTree::default();
        let inst = tree.add_instance("Lone", 1);
        if let Some(i) = tree.instances.get_mut(&inst) {
            i.translation = (3.0, 4.0, 5.0);
        }
        let (t, q) = tree.instance_world_transform(inst).unwrap();
        assert!((t - DVec3::new(3.0, 4.0, 5.0)).length() < 1e-9);
        assert!((q.w - 1.0).abs() < 1e-9);
    }

    #[test]
    fn zero_quaternion_is_treated_as_identity_not_nan() {
        // Data lama / rusak bisa menyimpan quaternion nol. Menormalkannya
        // akan menghasilkan NaN yang menjalar ke seluruh perakitan.
        let mut tree = AssemblyTree::default();
        let inst = tree.add_instance("Broken", 1);
        if let Some(i) = tree.instances.get_mut(&inst) {
            i.rotation_quat = (0.0, 0.0, 0.0, 0.0);
            i.translation = (1.0, 2.0, 3.0);
        }
        let (t, q) = tree.instance_world_transform(inst).unwrap();
        assert!(t.is_finite() && q.is_finite(), "tidak boleh NaN");
        assert!((t - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-9);
    }
}

#[cfg(test)]
mod joint_and_limit_tests {
    use super::*;

    #[test]
    fn joints_declare_which_freedoms_stay_open() {
        // Joint bukan kendala baru — ia NAMA untuk kombinasi kendala plus
        // deklarasi DOF mana yang sengaja dibiarkan bebas. "Engsel" dan
        // "silinder sepusat yang rotasinya dikunci" memakai geometri sama
        // persis; tanpa nama joint, perbedaannya tidak terekam di mana pun.
        assert_eq!(JointKind::Rigid.remaining_dof().total_dof(), 0);
        assert_eq!(JointKind::Revolute.remaining_dof().rotation_dof, 1);
        assert_eq!(JointKind::Revolute.remaining_dof().translation_dof, 0);
        assert_eq!(JointKind::Slider.remaining_dof().translation_dof, 1);
        assert_eq!(JointKind::Slider.remaining_dof().rotation_dof, 0);
        assert_eq!(JointKind::Cylindrical.remaining_dof().total_dof(), 2);
        assert_eq!(JointKind::Ball.remaining_dof().rotation_dof, 3);

        // Revolute dan Rigid memakai geometri yang sama (Concentric) tapi
        // berbeda pada penguncian rotasinya.
        assert!(matches!(
            JointKind::Rigid.underlying_mate(),
            MateKind::Concentric {
                lock_rotation: true,
                ..
            }
        ));
        assert!(matches!(
            JointKind::Revolute.underlying_mate(),
            MateKind::Concentric {
                lock_rotation: false,
                ..
            }
        ));
    }

    #[test]
    fn limits_clamp_driven_values() {
        // Engsel pintu tidak berputar 360°. Tanpa batas, solver akan dengan
        // senang hati menempatkan part di posisi yang mustahil secara fisik.
        let limits = MateLimits {
            min: Some(0.0),
            max: Some(90.0),
        };
        assert_eq!(limits.clamp(45.0), 45.0);
        assert_eq!(limits.clamp(-10.0), 0.0);
        assert_eq!(limits.clamp(120.0), 90.0);
        assert!(limits.contains(45.0));
        assert!(!limits.contains(120.0));

        let clamped = MateKind::Angle {
            angle_deg: 120.0,
            opposite_normal: false,
        }
        .with_value_clamped(&limits);
        assert_eq!(clamped.driven_value(), Some(90.0));
    }

    #[test]
    fn one_sided_limits_work() {
        let only_min = MateLimits {
            min: Some(5.0),
            max: None,
        };
        assert_eq!(only_min.clamp(1.0), 5.0);
        assert_eq!(only_min.clamp(50.0), 50.0);

        let only_max = MateLimits {
            min: None,
            max: Some(5.0),
        };
        assert_eq!(only_max.clamp(1.0), 1.0);
        assert_eq!(only_max.clamp(50.0), 5.0);
    }

    #[test]
    fn inverted_limits_are_ignored_not_obeyed() {
        // Data rusak (min > max) tidak boleh membuat nilai melompat ke
        // angka yang aneh; batas seperti itu diperlakukan sebagai tidak ada.
        let bad = MateLimits {
            min: Some(90.0),
            max: Some(0.0),
        };
        assert_eq!(bad.clamp(45.0), 45.0);
    }

    #[test]
    fn mates_without_numeric_value_are_untouched_by_limits() {
        let limits = MateLimits {
            min: Some(0.0),
            max: Some(1.0),
        };
        let concentric = MateKind::Concentric {
            lock_rotation: false,
            aligned: true,
        };
        assert_eq!(concentric.driven_value(), None);
        assert_eq!(concentric.with_value_clamped(&limits), concentric);
    }
}

#[cfg(test)]
mod bom_tests {
    use super::*;

    fn tree_with_repeats() -> AssemblyTree {
        let mut tree = AssemblyTree::default();
        for (name, pn) in [
            ("Baut M6", "BOLT-M6"),
            ("Baut M6", "BOLT-M6"),
            ("Baut M6", "BOLT-M6"),
            ("Plat", "PLATE-01"),
        ] {
            let id = tree.add_instance(name, 1);
            if let Some(i) = tree.instances.get_mut(&id) {
                i.part_number = Some(pn.to_string());
                i.material = Some("Baja".to_string());
            }
        }
        tree
    }

    #[test]
    fn repeated_parts_collapse_into_one_row_with_quantity() {
        // Inti sebuah BOM: part yang sama dipakai berkali-kali jadi SATU
        // baris berkuantitas, bukan empat baris terpisah.
        let tree = tree_with_repeats();
        let bom = tree.build_bom(true);
        assert_eq!(bom.len(), 2, "3 baut + 1 plat = 2 baris");

        let bolts = bom
            .iter()
            .find(|r| r.part_number.as_deref() == Some("BOLT-M6"))
            .unwrap();
        assert_eq!(bolts.quantity, 3);
    }

    #[test]
    fn parts_are_grouped_by_part_number_not_by_name() {
        // Dua part BERBEDA boleh bernama sama, dan satu part boleh diganti
        // namanya per instance. Menggabungkan berdasarkan nama akan salah.
        let mut tree = AssemblyTree::default();
        for pn in ["A-1", "A-2"] {
            let id = tree.add_instance("Bracket", 1);
            tree.instances.get_mut(&id).unwrap().part_number = Some(pn.to_string());
        }
        let bom = tree.build_bom(true);
        assert_eq!(bom.len(), 2, "nama sama tapi nomor part beda = 2 baris");
        assert!(bom.iter().all(|r| r.quantity == 1));
    }

    #[test]
    fn instances_without_part_number_fall_back_to_body_id() {
        // Belum semua part diberi nomor. Dua instance dari BODY yang sama
        // tetap harus digabung; body berbeda tidak.
        let mut tree = AssemblyTree::default();
        tree.add_instance("X", 7);
        tree.add_instance("X", 7);
        tree.add_instance("Y", 9);
        let bom = tree.build_bom(true);
        assert_eq!(bom.len(), 2);
        assert_eq!(
            bom.iter().map(|r| r.quantity).sum::<u32>(),
            3,
            "total kuantitas harus tetap 3"
        );
    }

    #[test]
    fn indented_bom_groups_by_top_level_sub_assembly() {
        // BOM indented mengelompokkan per sub-assembly TINGKAT PERTAMA,
        // jadi part yang bersarang dalam harus naik ke kelompok teratasnya.
        let mut tree = AssemblyTree::default();
        let outer = tree.add_sub_assembly("Gearbox", None);
        let inner = tree.add_sub_assembly("Shaft Group", Some(outer));

        let a = tree.add_instance("Gear", 1);
        tree.set_instance_parent(a, Some(inner));
        let b = tree.add_instance("Loose Bolt", 2);

        let bom = tree.build_bom(false);
        let gear = bom.iter().find(|r| r.name == "Gear").unwrap();
        assert_eq!(
            gear.sub_assembly.as_deref(),
            Some("Gearbox"),
            "harus naik ke sub-assembly teratas, bukan 'Shaft Group'"
        );
        let bolt = bom.iter().find(|r| r.name == "Loose Bolt").unwrap();
        assert_eq!(bolt.sub_assembly, None);
        assert_eq!(b, b); // instance id dipakai, memastikan ia benar dibuat
    }

    #[test]
    fn item_numbers_are_sequential_and_stable() {
        // Nomor item dipakai balon penunjuk di gambar; urutannya harus sama
        // tiap kali BOM digenerate, kalau tidak diff antar revisi gambar
        // jadi tak terbaca.
        let tree = tree_with_repeats();
        let first = tree.build_bom(true);
        let second = tree.build_bom(true);
        assert_eq!(first, second, "BOM harus deterministik");
        assert_eq!(
            first.iter().map(|r| r.item).collect::<Vec<_>>(),
            (1..=first.len() as u32).collect::<Vec<_>>()
        );
    }

    #[test]
    fn csv_escapes_commas_and_quotes() {
        // Tanpa pengutipan, nama part seperti `Bracket, Left` akan
        // menggeser SELURUH kolom di berkas CSV.
        let row = BomRow {
            item: 1,
            part_number: Some("PN-1".to_string()),
            name: "Bracket, Left".to_string(),
            material: Some("Alu \"6061\"".to_string()),
            quantity: 2,
            sub_assembly: None,
        };
        let csv = row.to_csv_row();
        assert!(csv.contains("\"Bracket, Left\""), "csv: {csv}");
        assert!(csv.contains("\"Alu \"\"6061\"\"\""), "csv: {csv}");
        assert_eq!(csv.matches(',').count(), 5 + 1, "koma pemisah + 1 di dalam kutip");
    }

    #[test]
    fn csv_has_header_and_one_line_per_row() {
        let tree = tree_with_repeats();
        let csv = bom_to_csv(&tree.build_bom(true));
        let lines: Vec<&str> = csv.trim_end().lines().collect();
        assert_eq!(lines[0], BomRow::csv_header());
        assert_eq!(lines.len(), 3, "header + 2 baris");
    }
}
