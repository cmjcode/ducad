//! Hidden Line Removal EKSAK di atas topologi B-rep (P4.1).
//!
//! # Kenapa ada, padahal `hlr.rs` sudah ada
//!
//! [`crate::hlr`] bekerja di atas MESH: ia mengumpulkan siluet dari
//! perubahan orientasi normal segitiga, lalu menentukan garis tersembunyi
//! dengan menguji oklusi terhadap segitiga-segitiga itu. Konsekuensinya
//! melekat pada pendekatannya, bukan pada kualitas implementasinya:
//!
//! - Lingkaran ⌀20 keluar sebagai poligon puluhan sisi, bukan lingkaran.
//!   Di gambar kerja yang dicetak, itu terlihat bergerigi dan tidak bisa
//!   diberi dimensi diameter yang benar.
//! - Garis tersembunyi bergantung pada kerapatan tesselasi: mengubah
//!   toleransi mesh mengubah gambar tekniknya.
//! - Rusuk yang berimpit dengan permukaan sering salah diklasifikasi.
//!
//! Modul ini bekerja pada topologi B-rep (`backend::hlr_mesh`): garis
//! kandidat = edge logis, posisi siluet di-snap ke permukaan analitik.
//! Hasilnya kurva analitik: lingkaran tetap lingkaran.

// Implementasi backend — lihat `crate::backend`.
pub use crate::backend::hlr_exact::*;

/// Jenis garis hasil HLR, sesuai penggambaran ISO 128.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactLineKind {
    /// Rusuk tajam yang terlihat — garis tebal kontinu.
    VisibleSharp,
    /// Siluet permukaan lengkung yang terlihat — juga garis tebal.
    VisibleOutline,
    /// Rusuk tajam tersembunyi — garis putus-putus.
    HiddenSharp,
    /// Siluet tersembunyi — garis putus-putus.
    HiddenOutline,
}

impl ExactLineKind {
    /// Apakah garis ini digambar putus-putus.
    pub fn is_hidden(self) -> bool {
        matches!(
            self,
            ExactLineKind::HiddenSharp | ExactLineKind::HiddenOutline
        )
    }
}

/// Jenis kurva asal sebuah [`ExactCurve2D`]. Milik DUCAD supaya API publik tidak membocorkan tipe backend mana pun
/// (PLAN_MIGRASI_TRUCK §3: API publik beku dan netral-backend).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactCurveType {
    Line,
    Circle,
    Ellipse,
    Hyperbola,
    Parabola,
    BezierCurve,
    BSplineCurve,
    OffsetCurve,
    OtherCurve,
}

/// Satu kurva 2D hasil proyeksi, dalam koordinat bidang gambar (mm).
///
/// `kind` mempertahankan JENIS KURVA aslinya (garis, lingkaran, elips,
/// b-spline) — inilah perbedaan pokok dengan HLR berbasis mesh, yang hanya
/// bisa mengembalikan rantai segmen lurus. Ekspor DXF/PDF dapat menuliskan
/// entitas CIRCLE/ARC sungguhan alih-alih ratusan LINE.
#[derive(Debug, Clone)]
pub struct ExactCurve2D {
    pub kind: ExactLineKind,
    pub curve: ExactCurveType,
    /// Titik-titik hasil sampling untuk penggambaran. Kurva analitik tetap
    /// membawa `curve` aslinya di atas, jadi pemanggil yang butuh presisi
    /// (ekspor DXF) tidak harus memakai sampel ini.
    pub points: Vec<(f64, f64)>,
}

impl ExactCurve2D {
    pub fn start(&self) -> Option<(f64, f64)> {
        self.points.first().copied()
    }
    pub fn end(&self) -> Option<(f64, f64)> {
        self.points.last().copied()
    }
}

/// Hasil HLR satu tampak.
#[derive(Debug, Clone, Default)]
pub struct ExactHlrView {
    pub curves: Vec<ExactCurve2D>,
}

impl ExactHlrView {
    pub fn visible(&self) -> impl Iterator<Item = &ExactCurve2D> {
        self.curves.iter().filter(|c| !c.kind.is_hidden())
    }
    pub fn hidden(&self) -> impl Iterator<Item = &ExactCurve2D> {
        self.curves.iter().filter(|c| c.kind.is_hidden())
    }
    /// Berapa kurva yang tetap analitik (bukan garis lurus) — dipakai
    /// menguji bahwa lingkaran tidak tercacah jadi poligon.
    pub fn analytic_count(&self) -> usize {
        self.curves
            .iter()
            .filter(|c| !matches!(c.curve, ExactCurveType::Line))
            .count()
    }
}

