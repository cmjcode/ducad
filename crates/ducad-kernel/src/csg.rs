use anyhow::{anyhow, bail, Context, Result};
use glam::dvec3;
use opencascade::adhoc::AdHocShape;
use opencascade::angle::Angle;
use opencascade::primitives::{Face, IntoShape, Solid};

use crate::lock_kernel;
use crate::mesh::tessellate_shape;
use crate::profile::{build_spine_wire, build_wire, build_wire_at_z, build_wire_on_plane, PathSegment, Profile};
use crate::shape::{deep_clone, KernelShape};

/// Extrude profil pada bidang 3D sembarang (origin, u_axis, v_axis, normal) sepanjang `distance` mm
/// searah normal bidang.
pub fn extrude_profile_on_plane(
    profile: &Profile,
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    normal: [f64; 3],
    distance: f64,
) -> Result<KernelShape> {
    if distance.abs() < 1e-9 {
        bail!("jarak extrude harus tidak nol");
    }
    let _guard = lock_kernel();
    let wire = build_wire_on_plane(profile, origin, u_axis, v_axis, normal)?;
    let face = Face::from_wire(&wire);
    let norm_len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    let extrude_dir = if norm_len > 1e-6 {
        dvec3(
            (normal[0] / norm_len) * distance,
            (normal[1] / norm_len) * distance,
            (normal[2] / norm_len) * distance,
        )
    } else {
        dvec3(0.0, 0.0, distance)
    };
    let solid = face.extrude(extrude_dir);
    Ok(KernelShape::from_inner(solid.into_shape()))
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

/// Revolve profil di bidang XY mengelilingi sumbu 2D (`axis_origin`+
/// `axis_dir`, keduanya diangkat ke Z=0 — axis yang dipakai selalu di
/// bidang XY sama seperti sketch-nya, konsisten dengan `build_wire`).
/// `angle_degrees: None` = revolve penuh 360° (default binding
/// `Face::revolve`); revolve parsial (sudut kustom) DIDUKUNG lewat
/// `Some(derajat)` tapi belum ada UI-nya di Fase 8 putaran pertama —
/// lihat docs/PLAN.md.
pub fn revolve_profile(
    profile: &Profile,
    axis_origin: (f64, f64),
    axis_dir: (f64, f64),
    angle_degrees: Option<f64>,
) -> Result<KernelShape> {
    let dir_len = (axis_dir.0 * axis_dir.0 + axis_dir.1 * axis_dir.1).sqrt();
    if dir_len < 1e-9 {
        bail!("sumbu revolve tidak valid (dua titik axis sama/terlalu dekat)");
    }
    let _guard = lock_kernel();
    let wire = build_wire(profile)?;
    let face = Face::from_wire(&wire);
    let origin = dvec3(axis_origin.0, axis_origin.1, 0.0);
    let axis = dvec3(axis_dir.0, axis_dir.1, 0.0);
    let angle = angle_degrees.map(Angle::Degrees);
    let solid: Solid = face.try_revolve(origin, axis, angle)
        .map_err(|e| anyhow!("Operasi Revolve gagal: pastikan sumbu putar tidak memotong bagian dalam profil dan profil membentuk bidang tertutup ({e})"))?;
    Ok(KernelShape::from_inner(solid.into_shape()))
}

/// Loft antara 2 profil: `bottom` di `z = 0`, `top` diangkat ke
/// `z = height`. BUKAN loft lintas-workplane sungguhan (sketch DUCAD
/// cuma satu bidang XY) — cara paling jujur untuk "loft" tanpa
/// infrastruktur workplane yang belum ada, lihat docs/PLAN.md.
pub fn loft_profiles(bottom: &Profile, top: &Profile, height: f64) -> Result<KernelShape> {
    if height.abs() < 1e-9 {
        bail!("tinggi loft harus tidak nol");
    }
    let _guard = lock_kernel();
    let bottom_wire = build_wire_at_z(bottom, 0.0)?;
    let top_wire = build_wire_at_z(top, height)?;
    let solid = Solid::loft([&bottom_wire, &top_wire]);
    Ok(KernelShape::from_inner(solid.into_shape()))
}

/// Menyapu (sweep / pipe) profil 2D tertutup di sepanjang kurva jalur (spine wire) 3D.
/// Menghasilkan bentuk solid B-rep 3D baru.
pub fn sweep_profile_along_wire(
    profile: &Profile,
    spine_wire: &opencascade::primitives::Wire,
) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let profile_wire = build_wire(profile)?;
    let profile_face = Face::from_wire(&profile_wire);
    let shape = profile_face
        .pipe(spine_wire)
        .map_err(|e| anyhow!("Operasi 3D Sweep gagal: pastikan profil tertutup dan jalur kurva valid ({e})"))?;
    Ok(KernelShape::from_inner(shape))
}

/// Menyapu (sweep / pipe) profil 2D tertutup di sepanjang daftar segmen jalur (spine path) 3D.
pub fn sweep_profile_along_path(
    profile: &Profile,
    spine_segments: &[PathSegment],
) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let spine_wire = build_spine_wire(spine_segments)?;
    let profile_wire = build_wire(profile)?;
    let profile_face = Face::from_wire(&profile_wire);
    let shape = profile_face
        .pipe(&spine_wire)
        .map_err(|e| anyhow!("Operasi 3D Sweep gagal: pastikan profil tertutup dan jalur kurva valid ({e})"))?;
    Ok(KernelShape::from_inner(shape))
}

/// Menyapu (sweep / pipe) profil 2D tertutup yang didefinisikan pada bidang (workplane) di sepanjang jalur 3D.
pub fn sweep_profile_on_plane_along_path(
    profile: &Profile,
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    normal: [f64; 3],
    spine_segments: &[PathSegment],
) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let spine_wire = build_spine_wire(spine_segments)?;
    let profile_wire = build_wire_on_plane(profile, origin, u_axis, v_axis, normal)?;
    let profile_face = Face::from_wire(&profile_wire);
    let shape = profile_face
        .pipe(&spine_wire)
        .map_err(|e| anyhow!("Operasi 3D Sweep gagal: pastikan profil tertutup dan jalur kurva valid ({e})"))?;
    Ok(KernelShape::from_inner(shape))
}


/// Union (gabung material) dua shape. `.clean()` (`ShapeUpgrade_
/// UnifySameDomain` OCCT) di-panggil sesudahnya supaya face/edge yang
/// koplanar persis di sambungan boolean (mis. sisi kubus yang di-extrude
/// lurus keluar dari sisi lama) DIGABUNG jadi satu face, bukan tertinggal
/// sebagai dua face terpisah yang cuma bertemu di satu garis (terlihat
/// seperti "jahitan"/seam ganda di viewport walau geometrinya valid).
pub fn union(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let mut merged = a
        .inner()
        .union(b.inner())
        .context("gagal menggabungkan (union) dua shape")?
        .shape;
    merged.clean();
    Ok(KernelShape::from_inner(merged))
}

/// Subtract (`a` dikurangi `b`) — lihat catatan `.clean()` di `union`.
pub fn subtract(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let mut result = a
        .inner()
        .subtract(b.inner())
        .context("gagal mengurangi (subtract) dua shape")?
        .shape;
    result.clean();
    Ok(KernelShape::from_inner(result))
}

/// Boolean intersect (irisan) dua shape — cuma sisakan volume yang
/// tumpang-tindih. `opencascade-rs` 0.2.0 tidak expose `.intersect()` di
/// `Shape` publik seperti union/subtract (cuma di `AdHocShape`, wrapper
/// tipis di atas `BRepAlgoAPI_Common`) — di-deep_clone dulu (pola sama
/// dengan fillet/chamfer) supaya `a`/`b` asli pemanggil tidak tersentuh,
/// lalu dibungkus `AdHocShape` sekali pakai untuk akses `.intersect()`.
pub fn intersect(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let cloned = deep_clone(a.inner())?;
    let mut adhoc = AdHocShape(cloned);
    adhoc
        .intersect(b.inner())
        .context("gagal menghitung irisan (intersect) dua shape")?;
    // Pakai `tessellate_shape` (helper privat, TIDAK mengunci sendiri) —
    // bukan `KernelShape::tessellate()` publik, yang akan mencoba
    // `lock_kernel()` lagi selagi `_guard` di atas masih dipegang (Mutex
    // std tidak reentrant, akan deadlock).
    if tessellate_shape(&adhoc.0).triangle_count() == 0 {
        bail!("intersect: kedua shape tidak bersinggungan (hasil kosong)");
    }
    Ok(KernelShape::from_inner(adhoc.0))
}

/// Operasi Emboss (timbul) atau Deboss (ukiran tenggelam / cut) untuk satu atau banyak profil pada bidang 3D.
///
/// - `base_shape`: Bodi 3D yang akan dikenai emboss/deboss (opsional).
/// - `profiles`: Daftar profil 2D tertutup (misal teks atau logo).
/// - `origin`, `u_axis`, `v_axis`, `normal`: Posisi & orientasi bidang sketsa.
/// - `depth`: Tinggi timbul (emboss) atau kedalaman ukiran (deboss) dalam mm (harus > 0).
/// - `is_deboss`: `false` untuk Emboss (timbul / union), `true` untuk Deboss (ukiran tenggelam / subtract).
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
