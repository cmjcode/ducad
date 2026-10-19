use anyhow::{bail, Result};
use glam::DVec3;

use crate::picking::ray::PickRay;
use crate::shape::KernelShape;

// Implementasi backend — lihat `crate::backend`.
pub use crate::backend::modify::*;

/// Arah pemilihan face yang dihilangkan untuk `shell_hollow` — face
/// TERJAUH ke arah ini yang dibuang (mis. `PosZ` membuang face atas,
/// menyisakan wadah terbuka ke atas). Hanya 1 face; hollow multi-face
/// (mis. buka 2 sisi sekaligus) belum didukung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

/// Face yang dibuka (dibuang) operasi shell.
#[derive(Debug, Clone, Copy)]
pub enum ShellOpening<'a> {
    /// Indeks [`crate::topo::enumerate_faces`].
    Indices(&'a [usize]),
    /// Ray picking GUI (bisa >1 — mis. buka 2 sisi sekaligus).
    Rays(&'a [PickRay]),
    /// Satu face terjauh ke arah sumbu ini (mis. `NegX` = buka sisi kiri).
    Farthest(Direction),
}

/// Error bertipe: kedalaman rongga menembus dinding dasar. Engine
/// mengenalinya lewat `downcast_ref` (pola sama dengan `EmptyIntersection`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellDepthTooDeep {
    pub depth: f64,
    /// Kedalaman terbesar yang masih menyisakan material di dasar rongga.
    pub max_depth: f64,
}

impl std::fmt::Display for ShellDepthTooDeep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "kedalaman rongga {} mm harus lebih kecil dari {:.3} mm (tinggi body dikurangi tebal dinding); isi 0 untuk rongga penuh",
            self.depth, self.max_depth
        )
    }
}

impl std::error::Error for ShellDepthTooDeep {}

pub(crate) fn check_shell_thickness(thickness: f64) -> Result<()> {
    if thickness.is_nan() || thickness <= 0.0 {
        bail!("tebal shell harus > 0");
    }
    Ok(())
}

/// Buat salinan solid 3D dalam susunan grid linier 3D sepanjang sumbu X, Y, dan Z.
/// `count_x`, `count_y`, `count_z`: jumlah item di tiap arah (minimal 1).
/// `pitch_x`, `pitch_y`, `pitch_z`: jarak pitch antar item (mm).
/// Mengembalikan HANYA solid salinan baru (tidak termasuk solid asli pada indeks (0, 0, 0)).
pub fn linear_pattern_shape(
    shape: &KernelShape,
    count_x: usize,
    pitch_x: f64,
    count_y: usize,
    pitch_y: f64,
    count_z: usize,
    pitch_z: f64,
) -> Result<Vec<KernelShape>> {
    let cx = count_x.max(1);
    let cy = count_y.max(1);
    let cz = count_z.max(1);

    let mut result = Vec::new();
    for iz in 0..cz {
        for iy in 0..cy {
            for ix in 0..cx {
                if ix == 0 && iy == 0 && iz == 0 {
                    continue;
                }
                let dx = ix as f64 * pitch_x;
                let dy = iy as f64 * pitch_y;
                let dz = iz as f64 * pitch_z;
                result.push(crate::shape::translate_shape(shape, dx, dy, dz)?);
            }
        }
    }
    Ok(result)
}

/// Buat salinan solid 3D dalam susunan melingkar (Circular Pattern 3D) mengelilingi sumbu putar poros 3D.
/// `pivot`: titik pada sumbu putar (x, y, z).
/// `axis`: vektor arah sumbu poros putar (x, y, z).
/// `count`: jumlah TOTAL item (minimal 2).
/// `total_angle_rad`: rentang sudut total (mis. 2*PI untuk 360° penuh).
/// Mengembalikan HANYA solid salinan baru (tidak termasuk solid asli pada k=0).
pub fn circular_pattern_shape(
    shape: &KernelShape,
    pivot: (f64, f64, f64),
    axis: (f64, f64, f64),
    count: usize,
    total_angle_rad: f64,
) -> Result<Vec<KernelShape>> {
    if count <= 1 {
        return Ok(Vec::new());
    }

    let is_full_circle = (total_angle_rad.abs() - std::f64::consts::TAU).abs() < 1e-4;
    let step_angle = if is_full_circle {
        total_angle_rad / count as f64
    } else {
        total_angle_rad / (count - 1) as f64
    };

    let mut result = Vec::new();
    for k in 1..count {
        let angle = k as f64 * step_angle;
        result.push(crate::shape::rotate_shape(shape, pivot, axis, angle)?);
    }
    Ok(result)
}

/// Buat tulang penguat beruntun (Polyline / Multi-segment Ribs) dari sekumpulan titik `points`.
pub fn create_rib_from_curve(
    shape: &KernelShape,
    points: &[DVec3],
    normal_dir: DVec3,
    thickness: f64,
    depth: f64,
    draft_angle_deg: Option<f64>,
) -> Result<KernelShape> {
    if points.len() < 2 {
        bail!("pembuatan tulang penguat butuh minimal 2 titik");
    }
    let mut current_shape = crate::shape::clone_shape(shape)?;
    for window in points.windows(2) {
        current_shape = create_rib(
            &current_shape,
            window[0],
            window[1],
            normal_dir,
            thickness,
            depth,
            draft_angle_deg,
        )?;
    }
    Ok(current_shape)
}

