//! Fase 3 — jembatan dokumen 3D (`ducad-core::Document`) ke geometri
//! kernel (`ducad-kernel::KernelShape`), plus command undo-able untuk
//! operasi modeling (Extrude, Union/Subtract, Fillet/Chamfer semua tepi,
//! Shell/Hollow, Hapus Body).
//!
//! `ducad-core::Document` sengaja bebas dependensi kernel (lihat komentar
//! di crate itu) — jadi geometri B-rep sungguhan hidup DI LUAR `Document`,
//! di `ModelDoc::geometry`, sebuah `SecondaryMap` yang dikunci dengan
//! `BodyId` yang SAMA dengan yang dipakai `Document::bodies`. `ModelDoc`
//! itulah target generik `ducad_core::Command<T>` untuk seluruh command
//! di modul ini — bukan `Document` langsung — karena command butuh
//! memutasi keduanya (metadata + geometri) sebagai satu langkah undo.
//!
//! Konsisten dengan `ducad_sketch::DeleteEntities`: `BodyId` TIDAK stabil
//! lintas undo/redo (slotmap tidak menjamin key lama bisa dipakai lagi) —
//! body yang dihapus lalu di-undo muncul kembali dengan id baru. Pemanggil
//! (UI) diharapkan mengosongkan seleksi body setelah operasi destruktif.

use ducad_core::{BodyId, Command, Document};
use ducad_kernel::{self, KernelMesh, KernelShape};
use slotmap::SecondaryMap;

/// Geometri kernel satu body — pasangan shape B-rep + mesh hasil
/// tessellation-nya (mesh di-cache di sini, bukan dihitung ulang tiap
/// frame render). `edge_dims` (fitur "Tampilkan Semua Ukuran", checkbox
/// ruler properties) di-cache dengan pola yang sama: dihitung SEKALI saat
/// geometri body dibuat/berubah, bukan tiap frame render viewport.
pub struct BodyGeometry {
    pub shape: KernelShape,
    /// `Arc` supaya callback render (yang harus `'static`) bisa memegang
    /// mesh tanpa menyalin vertex tiap frame — dulu seluruh posisi di-clone
    /// per body per frame ke buffer gabungan.
    pub mesh: std::sync::Arc<KernelMesh>,
    /// Sidik jari ISI mesh, dihitung sekali. Kunci cache buffer GPU: dua
    /// body dengan mesh identik berbagi satu buffer dan jadi instance.
    pub mesh_fingerprint: u64,
    pub edge_dims: Vec<ducad_kernel::EdgeDimension>,
    pub edge_lines: Vec<([f32; 3], [f32; 3])>,
    pub vertices: Vec<[f32; 3]>,
}

impl BodyGeometry {
    pub fn from_shape(shape: KernelShape) -> Self {
        let mesh = shape.tessellate();
        Self::from_shape_with_mesh(shape, mesh)
    }

    /// Sama seperti `from_shape`, tapi `mesh` SUDAH dihitung sebelumnya
    /// (dipakai `import_worker` di ducad-app: mesh dihitung di thread
    /// latar belakang supaya UI tidak beku, lalu shape dibangun ulang di
    /// UI thread dari teks STEP — lihat pemanggilnya). `edge_dims` dan
    /// `edge_lines` tetap dihitung di sini karena hanya `shape` yang
    /// dikirim balik dari worker, bukan dimensi/garis tepinya.
    pub fn from_shape_with_mesh(shape: KernelShape, mesh: KernelMesh) -> Self {
        let edge_dims = ducad_kernel::edge_dimensions(&shape);
        let edge_lines = ducad_kernel::extract_shape_edges(&shape, Some(&mesh));
        let vertices = ducad_kernel::shape_vertices(&shape)
            .into_iter()
            .map(|(x, y, z)| [x as f32, y as f32, z as f32])
            .collect();
        let mesh_fingerprint = mesh_fingerprint(&mesh);
        Self {
            shape,
            mesh: std::sync::Arc::new(mesh),
            mesh_fingerprint,
            edge_dims,
            edge_lines,
            vertices,
        }
    }

    /// Buat BodyGeometry langsung dari mesh (misal import STL) tanpa eksplorasi dimensi rusuk B-Rep.
    pub fn from_mesh_direct(shape: KernelShape, mesh: KernelMesh) -> Self {
        let edge_lines = ducad_kernel::extract_shape_edges(&shape, Some(&mesh));
        let vertices = ducad_kernel::shape_vertices(&shape)
            .into_iter()
            .map(|(x, y, z)| [x as f32, y as f32, z as f32])
            .collect();
        let mesh_fingerprint = mesh_fingerprint(&mesh);
        Self {
            shape,
            mesh: std::sync::Arc::new(mesh),
            mesh_fingerprint,
            edge_dims: Vec::new(),
            edge_lines,
            vertices,
        }
    }
}

/// FNV-1a atas bit posisi, normal, dan indeks. Stabil lintas jalan program
/// (bukan `DefaultHasher`), sehingga key cache GPU tidak berubah-ubah.
fn mesh_fingerprint(mesh: &KernelMesh) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |v: u32| {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    feed(mesh.positions.len() as u32);
    feed(mesh.indices.len() as u32);
    for p in &mesh.positions {
        feed(p[0].to_bits());
        feed(p[1].to_bits());
        feed(p[2].to_bits());
    }
    for n in &mesh.normals {
        feed(n[0].to_bits());
        feed(n[1].to_bits());
        feed(n[2].to_bits());
    }
    for i in &mesh.indices {
        feed(*i);
    }
    h
}

/// Dokumen 3D lengkap: metadata body (`ducad-core::Document`) + geometri
/// kernel-nya, dikunci `BodyId` yang sama. Lihat catatan modul.
#[derive(Default)]
pub struct ModelDoc {
    pub doc: Document,
    pub geometry: SecondaryMap<BodyId, BodyGeometry>,
}

/// Tambah satu body baru dari geometri yang SUDAH dihitung (dry-run sudah
/// selesai di pemanggil — lihat `apply_constraint` di `ducad-app` untuk
/// pola yang sama: hitung dulu, baru masuk undo stack kalau sukses).
pub struct AddSolidCommand {
    label: String,
    pending: Option<BodyGeometry>,
    id: Option<BodyId>,
}

impl AddSolidCommand {
    pub fn new(label: impl Into<String>, geometry: BodyGeometry) -> Self {
        Self {
            label: label.into(),
            pending: Some(geometry),
            id: None,
        }
    }
}

impl Command<ModelDoc> for AddSolidCommand {
    fn name(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, model: &mut ModelDoc) {
        if let Some(geo) = self.pending.take() {
            let id = model.doc.add_body(self.label.clone());
            model.geometry.insert(id, geo);
            self.id = Some(id);
        }
    }

    fn revert(&mut self, model: &mut ModelDoc) {
        if let Some(id) = self.id.take() {
            model.doc.bodies.remove(id);
            if let Some(geo) = model.geometry.remove(id) {
                self.pending = Some(geo);
            }
        }
    }
}

/// Tambah beberapa body baru sekaligus dalam 1 langkah undo/redo (dipakai oleh Pattern 3D).
pub struct AddMultipleSolidsCommand {
    label: String,
    pending: Option<Vec<(String, BodyGeometry)>>,
    ids: Option<Vec<BodyId>>,
}

impl AddMultipleSolidsCommand {
    pub fn new(label: impl Into<String>, bodies: Vec<(String, BodyGeometry)>) -> Self {
        Self {
            label: label.into(),
            pending: Some(bodies),
            ids: None,
        }
    }

    pub fn created_ids(&self) -> &[BodyId] {
        self.ids.as_deref().unwrap_or(&[])
    }
}

impl Command<ModelDoc> for AddMultipleSolidsCommand {
    fn name(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, model: &mut ModelDoc) {
        if let Some(items) = self.pending.take() {
            let mut created = Vec::with_capacity(items.len());
            for (name, geo) in items {
                let id = model.doc.add_body(name);
                model.geometry.insert(id, geo);
                created.push(id);
            }
            self.ids = Some(created);
        }
    }

    fn revert(&mut self, model: &mut ModelDoc) {
        if let Some(ids) = self.ids.take() {
            let mut pending = Vec::with_capacity(ids.len());
            for id in ids {
                if let Some(body) = model.doc.bodies.remove(id) {
                    if let Some(geo) = model.geometry.remove(id) {
                        pending.push((body.name, geo));
                    }
                }
            }
            self.pending = Some(pending);
        }
    }
}

/// Ganti geometri SATU body yang sudah ada dengan hasil baru (dipakai
/// Fillet/Chamfer semua tepi, Shell/Hollow) — `BodyId` tetap sama, cuma
/// isinya ditukar. `apply`/`revert` sengaja identik: keduanya cuma
/// menukar geometri yang tersimpan di `pending` dengan yang ada di peta
/// (`SecondaryMap::insert` mengembalikan nilai lama), jadi memanggilnya
/// dua kali berturut-turut kembali ke keadaan semula — pas untuk
/// apply/revert/redo yang simetris.
pub struct ReplaceGeometryCommand {
    label: &'static str,
    id: BodyId,
    pending: Option<BodyGeometry>,
}

impl ReplaceGeometryCommand {
    pub fn new(label: &'static str, id: BodyId, new_geometry: BodyGeometry) -> Self {
        Self {
            label,
            id,
            pending: Some(new_geometry),
        }
    }

    fn swap(&mut self, model: &mut ModelDoc) {
        if let Some(incoming) = self.pending.take() {
            if let Some(previous) = model.geometry.insert(self.id, incoming) {
                self.pending = Some(previous);
            }
        }
    }
}

impl Command<ModelDoc> for ReplaceGeometryCommand {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, model: &mut ModelDoc) {
        self.swap(model);
    }
    fn revert(&mut self, model: &mut ModelDoc) {
        self.swap(model);
    }
}

/// Ganti material SATU body yang sudah ada (Matte Plastic, Glossy Plastic, Anodized Aluminum, Chrome, Glass, Custom).
/// Mendukung undo/redo simetris.
pub struct SetBodyMaterialCommand {
    label: &'static str,
    id: BodyId,
    pending: Option<ducad_core::Material>,
}

impl SetBodyMaterialCommand {
    pub fn new(label: &'static str, id: BodyId, material: ducad_core::Material) -> Self {
        Self {
            label,
            id,
            pending: Some(material),
        }
    }

    fn swap(&mut self, model: &mut ModelDoc) {
        if let Some(incoming) = self.pending.take() {
            if let Some(body) = model.doc.bodies.get_mut(self.id) {
                let previous = std::mem::replace(&mut body.material, incoming);
                self.pending = Some(previous);
                model.doc.dirty = true;
            }
        }
    }
}

impl Command<ModelDoc> for SetBodyMaterialCommand {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, model: &mut ModelDoc) {
        self.swap(model);
    }
    fn revert(&mut self, model: &mut ModelDoc) {
        self.swap(model);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanKind {
    Union,
    Subtract,
    /// Irisan (cuma volume yang tumpang tindih) — Fase 8, lewat
    /// `ducad_kernel::intersect`.
    Intersect,
}

/// Sebelum di-apply: hasil sudah dihitung (dry-run), siap dipasang.
/// Setelah di-apply: body A & B sudah lenyap dari `ModelDoc`, datanya
/// (untuk revert) disimpan di sini.
#[allow(clippy::large_enum_variant)]
enum BooleanState {
    Pending(BodyGeometry),
    Applied {
        result_id: BodyId,
        a_body: ducad_core::Body,
        a_geo: BodyGeometry,
        b_body: ducad_core::Body,
        b_geo: BodyGeometry,
    },
}

/// Union/Subtract/Intersect dua body jadi satu body hasil — A & B dihapus,
/// hasil masuk sebagai body baru.
pub struct BooleanCommand {
    label: &'static str,
    result_name: String,
    a: BodyId,
    b: BodyId,
    state: Option<BooleanState>,
}

impl BooleanCommand {
    /// Hitung hasil boolean SEKARANG (dry-run) — mengembalikan `Err` kalau
    /// salah satu body tak ada geometrinya atau operasi kernel gagal,
    /// tanpa menyentuh `model` sama sekali.
    pub fn try_new(
        model: &ModelDoc,
        kind: BooleanKind,
        label: &'static str,
        result_name: impl Into<String>,
        a: BodyId,
        b: BodyId,
    ) -> Result<Self, String> {
        let geo_a = model.geometry.get(a).ok_or("Body A tidak ditemukan")?;
        let geo_b = model.geometry.get(b).ok_or("Body B tidak ditemukan")?;
        // Satu implementasi dengan Session (P0.8): `compute::boolean` juga
        // memeriksa validitas & volume hasil.
        let result_geo = crate::compute::boolean(&geo_a.shape, &geo_b.shape, kind).map_err(|e| {
            let own = format!("{label} gagal: ");
            if e.message.starts_with(&own) {
                e.message
            } else {
                format!("{own}{}", e.message)
            }
        })?;
        Ok(Self {
            label,
            result_name: result_name.into(),
            a,
            b,
            state: Some(BooleanState::Pending(result_geo)),
        })
    }
}

impl Command<ModelDoc> for BooleanCommand {
    fn name(&self) -> &str {
        self.label
    }

    fn apply(&mut self, model: &mut ModelDoc) {
        let Some(BooleanState::Pending(_)) = &self.state else {
            return;
        };
        let Some(BooleanState::Pending(result_geo)) = self.state.take() else {
            unreachable!()
        };
        let (Some(a_body), Some(a_geo), Some(b_body), Some(b_geo)) = (
            model.doc.bodies.remove(self.a),
            model.geometry.remove(self.a),
            model.doc.bodies.remove(self.b),
            model.geometry.remove(self.b),
        ) else {
            return;
        };
        let result_id = model.doc.add_body(self.result_name.clone());
        model.geometry.insert(result_id, result_geo);
        self.state = Some(BooleanState::Applied {
            result_id,
            a_body,
            a_geo,
            b_body,
            b_geo,
        });
    }

    fn revert(&mut self, model: &mut ModelDoc) {
        let Some(BooleanState::Applied { .. }) = &self.state else {
            return;
        };
        let Some(BooleanState::Applied {
            result_id,
            a_body,
            a_geo,
            b_body,
            b_geo,
        }) = self.state.take()
        else {
            unreachable!()
        };
        model.doc.bodies.remove(result_id);
        let result_geo = model.geometry.remove(result_id);

        // BodyId lama tidak bisa dipakai lagi (konvensi slotmap yang sama
        // dengan `ducad_sketch::DeleteEntities`) — perbarui `self.a`/`b`
        // supaya `apply` (redo) berikutnya menghapus id yang BENAR.
        self.a = model.doc.bodies.insert(a_body);
        model.geometry.insert(self.a, a_geo);
        self.b = model.doc.bodies.insert(b_body);
        model.geometry.insert(self.b, b_geo);

        if let Some(result_geo) = result_geo {
            self.state = Some(BooleanState::Pending(result_geo));
        }
    }
}

/// Hapus satu body (undo-able).
pub struct DeleteBodyCommand {
    id: BodyId,
    stash: Option<(ducad_core::Body, BodyGeometry)>,
}

impl DeleteBodyCommand {
    pub fn new(id: BodyId) -> Self {
        Self { id, stash: None }
    }
}

impl Command<ModelDoc> for DeleteBodyCommand {
    fn name(&self) -> &str {
        "Hapus Body"
    }
    fn apply(&mut self, model: &mut ModelDoc) {
        let (Some(body), Some(geo)) = (model.doc.bodies.remove(self.id), model.geometry.remove(self.id)) else {
            return;
        };
        self.stash = Some((body, geo));
    }
    fn revert(&mut self, model: &mut ModelDoc) {
        if let Some((body, geo)) = self.stash.take() {
            let new_id = model.doc.bodies.insert(body);
            model.geometry.insert(new_id, geo);
            self.id = new_id;
        }
    }
}

/// Isi varian `Applied` di-`Box` terpisah: `BodyGeometry` menyimpan
/// `KernelShape` + `KernelMesh` sehingga varian itu jauh lebih besar dari
/// `Pending`, dan enum tanpa box akan memakai ukuran varian terbesar untuk
/// SETIAP nilai (termasuk yang masih `Pending`).
struct SplitBodyApplied {
    orig_body: ducad_core::Body,
    orig_geo: BodyGeometry,
    result_ids: Vec<BodyId>,
    result_names: Vec<String>,
}

enum SplitBodyState {
    Pending(Vec<(String, BodyGeometry)>),
    Applied(Box<SplitBodyApplied>),
}

/// Split satu body menjadi N body terpisah (biasanya 2 body).
/// Body sumber dihapus, dan N body hasil ditambahkan ke `ModelDoc`.
/// Mendukung penuh Undo / Redo.
pub struct SplitBodyCommand {
    label: &'static str,
    target_id: BodyId,
    state: Option<SplitBodyState>,
}

impl SplitBodyCommand {
    pub fn new(
        target_id: BodyId,
        result_bodies: Vec<(String, BodyGeometry)>,
    ) -> Self {
        Self {
            label: "Split Body",
            target_id,
            state: Some(SplitBodyState::Pending(result_bodies)),
        }
    }

    /// ID body hasil yang baru saja dibuat oleh command ini.
    pub fn result_ids(&self) -> &[BodyId] {
        if let Some(SplitBodyState::Applied(applied)) = &self.state {
            &applied.result_ids
        } else {
            &[]
        }
    }
}

impl Command<ModelDoc> for SplitBodyCommand {
    fn name(&self) -> &str {
        self.label
    }

    fn apply(&mut self, model: &mut ModelDoc) {
        let Some(SplitBodyState::Pending(_)) = &self.state else {
            return;
        };
        let Some(SplitBodyState::Pending(new_bodies)) = self.state.take() else {
            unreachable!()
        };
        let (Some(orig_body), Some(orig_geo)) = (
            model.doc.bodies.remove(self.target_id),
            model.geometry.remove(self.target_id),
        ) else {
            return;
        };

        let mut result_ids = Vec::with_capacity(new_bodies.len());
        let mut result_names = Vec::with_capacity(new_bodies.len());

        for (name, geo) in new_bodies {
            let id = model.doc.add_body(name.clone());
            model.geometry.insert(id, geo);
            result_ids.push(id);
            result_names.push(name);
        }

        self.state = Some(SplitBodyState::Applied(Box::new(SplitBodyApplied {
            orig_body,
            orig_geo,
            result_ids,
            result_names,
        })));
    }

    fn revert(&mut self, model: &mut ModelDoc) {
        let Some(SplitBodyState::Applied(_)) = &self.state else {
            return;
        };
        let Some(SplitBodyState::Applied(applied)) = self.state.take() else {
            unreachable!()
        };
        let SplitBodyApplied {
            orig_body,
            orig_geo,
            result_ids,
            result_names,
        } = *applied;

        let mut pending = Vec::with_capacity(result_ids.len());
        for (id, name) in result_ids.into_iter().zip(result_names) {
            model.doc.bodies.remove(id);
            if let Some(geo) = model.geometry.remove(id) {
                pending.push((name, geo));
            }
        }

        // Kembalikan body awal
        self.target_id = model.doc.bodies.insert(orig_body);
        model.geometry.insert(self.target_id, orig_geo);

        self.state = Some(SplitBodyState::Pending(pending));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_kernel::Profile;
    #[test]
    fn test_split_body_command_undo_redo() {
        let mut model = ModelDoc::default();
        let circle_profile = Profile::Circle { center: (0.0, 0.0), radius: 10.0 };
        let shape = ducad_kernel::extrude_profile(&circle_profile, 40.0).unwrap();
        let initial_id = model.doc.add_body("Original Cylinder");
        model.geometry.insert(initial_id, BodyGeometry::from_shape(shape));

        assert_eq!(model.doc.bodies.len(), 1);

        // Split via kernel
        let orig_shape = &model.geometry.get(initial_id).unwrap().shape;
        let mut parts = ducad_kernel::split_body(
            orig_shape,
            glam::DVec3::new(0.0, 0.0, 20.0),
            glam::DVec3::new(0.0, 0.0, 1.0),
        )
        .unwrap();
        assert_eq!(parts.len(), 2);

        let p2 = parts.pop().unwrap();
        let p1 = parts.pop().unwrap();
        let result_bodies = vec![
            ("Original Cylinder (Bagian 1)".to_string(), BodyGeometry::from_shape(p1)),
            ("Original Cylinder (Bagian 2)".to_string(), BodyGeometry::from_shape(p2)),
        ];

        let mut cmd = SplitBodyCommand::new(initial_id, result_bodies);
        cmd.apply(&mut model);

        assert_eq!(model.doc.bodies.len(), 2);
        assert_eq!(model.geometry.len(), 2);

        // Revert (Undo)
        cmd.revert(&mut model);
        assert_eq!(model.doc.bodies.len(), 1);
        assert_eq!(model.geometry.len(), 1);

        // Re-apply (Redo)
        cmd.apply(&mut model);
        assert_eq!(model.doc.bodies.len(), 2);
        assert_eq!(model.geometry.len(), 2);
    }

    #[test]
    fn test_add_multiple_solids_command_undo_redo() {
        let mut model = ModelDoc::default();
        let circle_profile = Profile::Circle { center: (0.0, 0.0), radius: 5.0 };
        let shape1 = ducad_kernel::extrude_profile(&circle_profile, 10.0).unwrap();
        let shape2 = ducad_kernel::extrude_profile(&circle_profile, 20.0).unwrap();

        let items = vec![
            ("Solid 1".to_string(), BodyGeometry::from_shape(shape1)),
            ("Solid 2".to_string(), BodyGeometry::from_shape(shape2)),
        ];

        let mut cmd = AddMultipleSolidsCommand::new("Pattern 3D", items);
        cmd.apply(&mut model);

        assert_eq!(model.doc.bodies.len(), 2);
        assert_eq!(model.geometry.len(), 2);
        assert_eq!(cmd.created_ids().len(), 2);

        // Revert (Undo)
        cmd.revert(&mut model);
        assert_eq!(model.doc.bodies.len(), 0);
        assert_eq!(model.geometry.len(), 0);

        // Re-apply (Redo)
        cmd.apply(&mut model);
        assert_eq!(model.doc.bodies.len(), 2);
        assert_eq!(model.geometry.len(), 2);
    }

}
