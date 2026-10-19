

// Implementasi khusus backend (OCCT/truck) — lihat `crate::backend`.
pub use crate::backend::picking_face::*;

/// Toleransi geometris `BRepIntCurveSurface_Inter` untuk face-picking
/// interaktif — lebih longgar dari default upstream (`0.0001`, lihat
/// `vendor/README.md`). TERBUKTI (test terisolasi) TIDAK CUKUP sendirian
/// utk kasus ray oblique pada wajah sweep/samping (root cause BUKAN
/// toleransi — lihat `resolve_planar_face_along_ray_fallback` di bawah)
/// tapi tetap dipertahankan sebagai margin aman kecil, tidak merugikan.
#[cfg_attr(backend_truck, allow(dead_code))] // dipakai backend OCCT; truck menyusul (F3/F4)
pub(crate) const FACE_PICK_TOLERANCE_MM: f64 = 0.01;

/// Tipe permukaan geometris analitik OCCT di balik sebuah `Face`, hasil
/// klasifikasi `Face::surface_kind()` (nama kelas dinamis C++ `Geom_*`).
/// Fase 1: sekadar deteksi/label — belum dipakai fitur apa pun, disiapkan
/// utk fitur mendatang yang berperilaku beda tergantung tipe face (mis.
/// deteksi smart boolean, hint UI khusus silinder/bola).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    /// Permukaan analitik lain (mis. torus non-standar) atau permukaan
    /// bebas/non-analitik (mis. B-Spline, hasil loft/sweep).
    Other,
}

impl From<&str> for SurfaceKind {
    /// Petakan nama kelas dinamis OCCT (keluaran `Face::surface_kind()`)
    /// ke varian enum. Nama yang tidak dikenali (termasuk permukaan
    /// bebas/non-analitik) jatuh ke `Other`, bukan error — klasifikasi ini
    /// sifatnya informasional, tidak boleh gagal.
    fn from(dynamic_type_name: &str) -> Self {
        match dynamic_type_name {
            "Geom_Plane" => SurfaceKind::Plane,
            "Geom_CylindricalSurface" => SurfaceKind::Cylinder,
            "Geom_ConicalSurface" => SurfaceKind::Cone,
            "Geom_SphericalSurface" => SurfaceKind::Sphere,
            "Geom_ToroidalSurface" => SurfaceKind::Torus,
            _ => SurfaceKind::Other,
        }
    }
}

/// Informasi hit face hasil picking 3D (titik hit, titik pusat centroid, normal keluar, dan loop poligon batas face).
#[derive(Debug, Clone, PartialEq)]
pub struct FaceHit {
    /// Titik hit langsung pada permukaan face (x, y, z)
    pub hit_point: (f64, f64, f64),
    /// Titik pusat / centroid aproksimasi face (x, y, z)
    pub centroid: (f64, f64, f64),
    /// Vektor normal satuan yang mengarah ke luar (outward normal) (x, y, z)
    pub normal: (f64, f64, f64),
    /// Tipe permukaan geometris analitik di balik face yang terpilih.
    pub surface_kind: SurfaceKind,
    /// Arah satuan yang dipakai gizmo push/pull (DUCAD Fase 4)
    pub pull_dir: (f64, f64, f64),
    /// Titik-titik pembatas sekeliling perimeter face dalam ruang 3D
    pub boundary_points: Vec<(f64, f64, f64)>,
    /// Indeks face dalam urutan `Shape::faces()` — sama dengan urutan
    /// `KernelMesh::face_ranges`, sehingga segitiga face ini di mesh body
    /// bisa disorot secara eksak. `None` bila face tak ditemukan lagi.
    pub face_index: Option<usize>,
    /// Radius permukaan analitik (mm): silinder/kerucut (`cylinder_or_cone_radius`)
    /// atau bola. `None` untuk bidang datar/permukaan bebas. Dipakai gizmo
    /// tarik-sisi GUI untuk menampilkan radius HASIL (`R 101.2`) alih-alih
    /// hanya selisih `ΔR`.
    pub surface_radius: Option<f64>,
    /// Kerangka radial permukaan analitik, dipakai pratinjau tarik-sisi GUI
    /// agar tiap vertex digeser MENJAUHI/MENDEKATI sumbu (membesarkan radius),
    /// bukan ditranslasikan searah `pull_dir` seperti face datar.
    /// `None` untuk bidang datar/permukaan lain.
    pub radial_axis: Option<RadialAxis>,
}

/// Sumbu/pusat radial permukaan lengkung (lihat `FaceHit::radial_axis`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialAxis {
    /// Titik pada sumbu silinder/kerucut, atau pusat bola.
    pub origin: (f64, f64, f64),
    /// Arah sumbu satuan; `(0,0,0)` untuk bola (radial dari satu titik).
    pub dir: (f64, f64, f64),
}

impl FaceHit {
    /// Titik anchor gizmo push/pull: SELALU `hit_point` — titik yang
    /// diklik pengguna, pasti berada DI PERMUKAAN face dan di layar.
    ///
    /// Dulu face datar memakai `centroid`, yaitu rata-rata titik sampel
    /// batas. Titik sampel menumpuk di tepi lengkung dan poligon batas bisa
    /// cekung, sehingga rata-rata itu dapat jatuh jauh di luar face (mis.
    /// sisi samping body hasil fuse) — gizmo tampil di ruang kosong dan tak
    /// terjangkau. Untuk selimut silinder/bola `centroid` bahkan jatuh di
    /// sumbu/pusat, di dalam material.
    pub fn gizmo_anchor(&self) -> (f64, f64, f64) {
        self.hit_point
    }
}


