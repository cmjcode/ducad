//! Hidden Line Removal EKSAK di atas `HLRBRep_Algo` (P4.1).
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
//! Modul ini memakai algoritma HLR sungguhan milik OCCT, yang bekerja pada
//! topologi B-rep. Hasilnya kurva analitik: lingkaran tetap lingkaran.

use anyhow::{bail, Result};
use glam::DVec3;
use opencascade::primitives::EdgeType;

use crate::lock_kernel;
use crate::shape::KernelShape;

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

/// Satu kurva 2D hasil proyeksi, dalam koordinat bidang gambar (mm).
///
/// `kind` mempertahankan JENIS KURVA aslinya (garis, lingkaran, elips,
/// b-spline) — inilah perbedaan pokok dengan HLR berbasis mesh, yang hanya
/// bisa mengembalikan rantai segmen lurus. Ekspor DXF/PDF dapat menuliskan
/// entitas CIRCLE/ARC sungguhan alih-alih ratusan LINE.
#[derive(Debug, Clone)]
pub struct ExactCurve2D {
    pub kind: ExactLineKind,
    pub curve: EdgeType,
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
            .filter(|c| !matches!(c.curve, EdgeType::Line))
            .count()
    }
}

/// Jalankan HLR eksak pada `shape` dari arah pandang `view_dir`.
///
/// `up` menentukan orientasi vertikal gambar; ia tidak boleh sejajar dengan
/// arah pandang.
pub fn extract_exact_hlr(
    shape: &KernelShape,
    view_dir: (f64, f64, f64),
    up: (f64, f64, f64),
) -> Result<ExactHlrView> {
    let _guard = lock_kernel();
    let dir = DVec3::new(view_dir.0, view_dir.1, view_dir.2);
    let up_v = DVec3::new(up.0, up.1, up.2);
    if dir.length() < 1e-9 || up_v.length() < 1e-9 {
        bail!("arah pandang dan vektor atas HLR tidak boleh nol");
    }
    if dir.normalize().cross(up_v.normalize()).length() < 1e-6 {
        bail!("vektor atas HLR tidak boleh sejajar arah pandang");
    }

    let Some((vis_sharp, vis_out, hid_sharp, hid_out)) =
        shape.inner().hidden_line_removal(dir, up_v)
    else {
        bail!("HLR gagal untuk geometri ini");
    };

    let mut curves = Vec::new();
    for (compound, kind) in [
        (vis_sharp, ExactLineKind::VisibleSharp),
        (vis_out, ExactLineKind::VisibleOutline),
        (hid_sharp, ExactLineKind::HiddenSharp),
        (hid_out, ExactLineKind::HiddenOutline),
    ] {
        for edge in compound.edges() {
            // Hasil HLR berada di koordinat proyektor: bidang gambar adalah
            // XY dan Z cuma kedalaman, jadi Z dibuang.
            let pts: Vec<(f64, f64)> = edge
                .approximation_segments()
                .map(|p| (p.x, p.y))
                .collect();
            if pts.len() < 2 {
                continue;
            }
            curves.push(ExactCurve2D {
                kind,
                curve: edge.edge_type(),
                points: pts,
            });
        }
    }
    Ok(ExactHlrView { curves })
}
