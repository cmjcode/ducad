pub mod dimensions;
pub mod gizmo;
pub mod lines;

/// Tinggi handle gizmo extrude profil saat belum digeser (mm dari bidang sketch).
/// Dipakai juga sebagai nilai awal `gizmo_distance` saat drag dimulai agar
/// handle tidak melompat dari bawah pointer.
pub const GIZMO_IDLE_HEIGHT_MM: f64 = 18.0;
