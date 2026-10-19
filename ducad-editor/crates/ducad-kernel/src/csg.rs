use anyhow::{bail, Result};

use crate::profile::Profile;
use crate::shape::KernelShape;

// Implementasi khusus backend (OCCT/truck) — lihat `crate::backend`.
pub use crate::backend::csg::*;

/// Union banyak shape sekaligus (mis. pemotong pattern/hole sebelum satu
/// boolean ke target). Kosong → shape kosong; satu → shape itu sendiri.
///
/// Backend truck (PLAN_MIGRASI_TRUCK 5.4): bila semua bbox saling terpisah
/// digabung tanpa boolean (`union_apart`); selain itu union berpasangan
/// berbentuk pohon, tiap tingkat paralel di thread pekerja. Backend OCCT:
/// lipatan berurutan (kernel diserialkan `KERNEL_LOCK`).
pub fn union_all(shapes: Vec<KernelShape>) -> Result<KernelShape> {
    #[cfg(backend_truck)]
    {
        crate::backend::csg::union_many(shapes)
    }
    #[cfg(backend_occt)]
    {
        let mut it = shapes.into_iter();
        let Some(mut acc) = it.next() else {
            return Ok(KernelShape::empty());
        };
        for s in it {
            acc = union(&acc, &s)?;
        }
        Ok(acc)
    }
}

/// Seberapa jauh sebuah profil di-extrude, dan ke arah mana.
///
/// Sebelumnya hanya ada satu perilaku: maju sejauh `distance` dari bidang
/// sketsa. Padahal bos/potongan yang berpusat pada bidangnya adalah hal
/// biasa — dan mengerjakannya dengan dua extrude lalu union berarti membayar
/// satu operasi boolean penuh (yang bisa gagal sendiri) untuk sesuatu yang
/// sebetulnya cuma pergeseran titik awal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtrudeExtent {
    /// Maju sejauh `0..distance` dari bidang sketsa. Nilai negatif berarti
    /// mundur — perilaku yang sama dengan `extrude_profile_on_plane`.
    Blind(f64),
    /// Total setebal `length`, berpusat pada bidang sketsa (setengah maju,
    /// setengah mundur).
    Symmetric(f64),
    /// Panjang berbeda di tiap sisi bidang sketsa.
    TwoSided { forward: f64, backward: f64 },
}

impl ExtrudeExtent {
    /// Uraikan jadi (pergeseran titik awal sepanjang normal, panjang total).
    ///
    /// Semua varian diselesaikan sebagai SATU prisma: bidang profil digeser
    /// mundur lalu di-extrude sepanjang jumlahnya. Tidak ada boolean yang
    /// dibayar, dan hasilnya satu solid tanpa sambungan internal.
    fn resolve(self) -> (f64, f64) {
        match self {
            ExtrudeExtent::Blind(d) => {
                if d >= 0.0 {
                    (0.0, d)
                } else {
                    // Extrude mundur = mulai dari ujung mundur lalu maju.
                    (d, -d)
                }
            }
            ExtrudeExtent::Symmetric(len) => (-len.abs() * 0.5, len.abs()),
            ExtrudeExtent::TwoSided { forward, backward } => (-backward, forward + backward),
        }
    }

    /// Panjang total material yang dihasilkan.
    pub fn total_length(self) -> f64 {
        self.resolve().1
    }
}

/// Extrude profil pada bidang 3D sembarang dengan mode [`ExtrudeExtent`].
pub fn extrude_profile_extent(
    profile: &Profile,
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    normal: [f64; 3],
    extent: ExtrudeExtent,
) -> Result<KernelShape> {
    let (offset, length) = extent.resolve();
    if length.abs() < 1e-9 {
        bail!("panjang extrude harus tidak nol");
    }
    let norm_len =
        (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if norm_len < 1e-9 {
        bail!("normal bidang extrude degenerate");
    }
    let unit = [
        normal[0] / norm_len,
        normal[1] / norm_len,
        normal[2] / norm_len,
    ];
    let shifted_origin = [
        origin[0] + unit[0] * offset,
        origin[1] + unit[1] * offset,
        origin[2] + unit[2] * offset,
    ];
    extrude_profile_on_plane(profile, shifted_origin, u_axis, v_axis, normal, length)
}

/// Extrude profil di bidang XY sepanjang `distance` mm di sumbu Z.
pub fn extrude_profile(profile: &Profile, distance: f64) -> Result<KernelShape> {
    extrude_profile_on_plane(
        profile,
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        distance,
    )
}

/// Error bertipe untuk irisan kosong — supaya pemanggil (engine) bisa
/// membedakannya dari kegagalan kernel lain lewat `downcast_ref`, tanpa
/// mencocokkan teks pesan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyIntersection;

impl std::fmt::Display for EmptyIntersection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("intersect: kedua shape tidak bersinggungan (hasil kosong)")
    }
}

impl std::error::Error for EmptyIntersection {}

/// Operasi Emboss (timbul) atau Deboss (ukiran tenggelam / cut) untuk satu atau banyak profil pada bidang 3D.
///
/// - `base_shape`: Bodi 3D yang akan dikenai emboss/deboss (opsional).
/// - `profiles`: Daftar profil 2D tertutup (misal teks atau logo).
/// - `origin`, `u_axis`, `v_axis`, `normal`: Posisi & orientasi bidang sketsa.
/// - `depth`: Tinggi timbul (emboss) atau kedalaman ukiran (deboss) dalam mm (harus > 0).
/// - `is_deboss`: `false` untuk Emboss (timbul / union), `true` untuk Deboss (ukiran tenggelam / subtract).
#[allow(clippy::too_many_arguments)]
pub fn emboss_profiles_on_plane(
    base_shape: Option<&KernelShape>,
    profiles: &[Profile],
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    normal: [f64; 3],
    depth: f64,
    is_deboss: bool,
) -> Result<KernelShape> {
    if depth <= 0.0 {
        bail!("kedalaman emboss/deboss harus > 0");
    }
    if profiles.is_empty() {
        bail!("tidak ada profil untuk di-emboss/deboss");
    }

    let ext_distance = if is_deboss { -depth } else { depth };

    let mut tool_shape = extrude_profile_on_plane(&profiles[0], origin, u_axis, v_axis, normal, ext_distance)?;

    for p in &profiles[1..] {
        if let Ok(next_shape) = extrude_profile_on_plane(p, origin, u_axis, v_axis, normal, ext_distance) {
            if let Ok(joined) = union(&tool_shape, &next_shape) {
                tool_shape = joined;
            }
        }
    }

    if let Some(base) = base_shape {
        if is_deboss {
            subtract(base, &tool_shape)
        } else {
            match union(base, &tool_shape) {
                Ok(res) => Ok(res),
                Err(_) => Ok(tool_shape),
            }
        }
    } else {
        Ok(tool_shape)
    }
}
