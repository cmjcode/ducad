

// Implementasi khusus backend (OCCT/truck) — lihat `crate::backend`.
pub use crate::backend::picking_edge::*;

/// (titik hit terdekat di edge, polyline approksimasi edge itu utk
/// highlight render) — hasil `pick_edge`.
pub type EdgePickHit = ((f64, f64, f64), Vec<(f64, f64, f64)>);

/// (titik tengah dunia edge di arc-length setengah panjangnya, titik AWAL
/// dan AKHIR edge, panjang total edge) — satu entri per edge topologi
/// shape. `start`/`end` disertakan (bukan cuma titik tengah) supaya
/// pemanggil (ducad-app) bisa menghitung sudut layar rusuknya SETELAH
/// diproyeksikan kamera — label dimensi dengan begitu bisa disejajarkan ke
/// arah rusuknya sendiri, ikut berubah sudut saat kamera diputar, sama
/// seperti pill dimensi entitas sketsa 2D.
pub type EdgeDimension = ((f64, f64, f64), (f64, f64, f64), (f64, f64, f64), f64);

/// Hitung vektor normal keluar (outward radial normal) dari rusuk pada shape.
/// Mengambil rata-rata normal keluar dari face-face yang bertemu di rusuk ini,
/// lalu memproyeksikannya tegak lurus terhadap garis singgung rusuk (edge tangent).
/// Bekerja konsisten untuk sudut luar (convex) maupun sudut dalam (concave).
/// Titik/vektor 3D mentah di batas crate kernel. Sengaja tuple `f64`
/// polos, bukan `glam::DVec3`: `ducad-kernel` dipaku ke glam versi
/// kernel, jadi tipe glam tidak pernah boleh bocor ke crate pemanggil
/// (pola yang sama dengan `KernelMesh`/`Profile`).
pub type Vec3Raw = (f64, f64, f64);

/// Pasangan (titik pada rusuk, normal keluar di titik itu).
pub type EdgeNormalHit = (Vec3Raw, Vec3Raw);

