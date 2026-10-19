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



use crate::picking::face::SurfaceKind;

// Implementasi khusus backend (OCCT/truck) — lihat `crate::backend`.
pub use crate::backend::topo::*;

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

