use anyhow::{Context, Result};
use glam::dvec3;
use opencascade::primitives::Shape;

use crate::lock_kernel;
use crate::mesh::{tessellate_shape, KernelMesh};

/// Solid/shape B-rep OCCT. Field dalam sengaja privat — pemanggil di luar
/// crate ini hanya boleh membangunnya lewat fungsi di modul ini
/// (`extrude_profile`, `union`, `subtract`, dst) dan membacanya lewat
/// `tessellate`/`write_stl`, tidak pernah menyentuh tipe `opencascade`
/// langsung.
pub struct KernelShape(pub(crate) Shape);

unsafe impl Send for KernelShape {}

impl KernelShape {
    pub(crate) fn from_inner(shape: Shape) -> Self {
        KernelShape(shape)
    }

    pub(crate) fn inner(&self) -> &Shape {
        &self.0
    }

    /// Buat shape kosong (TopoDS_Compound kosong) — dipakai untuk mesh body murni (seperti impor STL).
    pub fn empty() -> Self {
        let _guard = lock_kernel();
        let empty_vec: Vec<&Shape> = Vec::new();
        let compound = opencascade::primitives::Compound::from_shapes(empty_vec);
        KernelShape(compound.into())
    }

    pub fn tessellate(&self) -> KernelMesh {
        let _guard = lock_kernel();
        tessellate_shape(&self.0)
    }

    pub fn write_stl(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let _guard = lock_kernel();
        self.0.write_stl(path)?;
        Ok(())
    }

    pub fn write_step(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let _guard = lock_kernel();
        self.0.write_step(path)?;
        Ok(())
    }

    /// Baca shape B-rep dari file STEP — kebalikan `write_step`. Dipakai
    /// Import STEP (Fase 5, `ducad-io`; Fase 7, `ducad-app::import_worker`)
    /// dan test/`deep_clone` internal.
    pub fn read_step(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let _guard = lock_kernel();
        Ok(KernelShape(
            Shape::read_step(path).context("read_step: gagal membaca STEP")?,
        ))
    }

    /// Baca shape dari file STL (biner/ASCII).
    pub fn read_stl(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let _guard = lock_kernel();
        Ok(KernelShape(
            Shape::read_stl(path).context("read_stl: gagal membaca STL")?,
        ))
    }

    /// Serialize B-rep ini jadi teks STEP AP214 (bukan mesh — topologi+
    /// geometri persis, sama presisi dengan file `.step` biasa). Dipakai
    /// `ducad-io` (Fase 5) untuk menyematkan body ke dalam SATU file
    /// native `.ducad` tanpa pernah menyentuh tipe `opencascade` — cuma
    /// String, sama seperti `KernelMesh` membungkus mesh sebagai `[f32;3]`
    /// mentah. Roundtrip lewat file sementara (sama trik dengan
    /// `deep_clone`) karena binding ini tidak expose serialisasi in-memory.
    pub fn to_step_string(&self) -> Result<String> {
        let _guard = lock_kernel();
        let path = temp_step_path("to-step-string");
        let result = (|| -> Result<String> {
            self.0
                .write_step(&path)
                .context("to_step_string: gagal menulis STEP sementara")?;
            std::fs::read_to_string(&path)
                .context("to_step_string: gagal membaca balik STEP sementara")
        })();
        let _ = std::fs::remove_file(&path);
        result
    }

    /// Kebalikan `to_step_string`.
    pub fn from_step_string(step: &str) -> Result<Self> {
        let _guard = lock_kernel();
        let path = temp_step_path("from-step-string");
        let result = (|| -> Result<Shape> {
            std::fs::write(&path, step)
                .context("from_step_string: gagal menulis STEP sementara")?;
            Shape::read_step(&path).context("from_step_string: gagal membaca balik STEP sementara")
        })();
        let _ = std::fs::remove_file(&path);
        result.map(KernelShape)
    }
}

/// Path file sementara unik (PID + timestamp nanosecond, sama pola dengan
/// yang dipakai `deep_clone` sebelumnya) — dipusatkan di sini supaya
/// `deep_clone`/`to_step_string`/`from_step_string` tidak menduplikasi
/// logika pembuatan nama file.
pub(crate) fn temp_step_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ducad-{tag}-{}-{}.step",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

/// Kloning in-memory cepat untuk `Shape` menggunakan binding `Clone` (TopoDS_Shape_to_owned)
/// tanpa pernah menyentuh file sementara di disk.
pub(crate) fn deep_clone(shape: &Shape) -> Result<Shape> {
    Ok(shape.clone())
}

/// Deep-clone publik sebuah shape — dipakai app untuk menyimpan snapshot
/// B-rep (mis. shape dasar SEBELUM rounding parametrik pertama, supaya
/// radius bisa diubah/di-nol-kan lagi dengan rebuild dari dasar) tanpa
/// membuka akses ke `deep_clone` internal maupun detail locking kernel.
pub fn clone_shape(shape: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    Ok(KernelShape(shape.0.clone()))
}

/// Geser shape sepanjang X/Y/Z dunia sejauh `(dx, dy, dz)` mm — dipakai
/// gizmo drag axis body 3D. Fungsional (tidak memutasi `shape` pemanggil):
/// `Shape` tidak `Clone`, jadi `deep_clone` dulu sama seperti
/// `fillet_all`/`chamfer_all`, tapi di sini transformasinya jauh lebih
/// murah — `set_global_translation` (API vendor `opencascade-0.2.0`,
/// sudah ada) cuma menggeser `Location` shape, TIDAK merombak B-rep sama
/// sekali (beda dari fillet/chamfer/boolean yang benar-benar membangun
/// ulang geometri). `dx`/`dy`/`dz` adalah delta, bukan posisi absolut —
/// pemanggil (gizmo di `ducad-app`) selalu menghitung ulang dari shape
/// ASLI sebelum drag dimulai (pola sama dgn gizmo extrude face lain),
/// jadi tidak ada akumulasi error floating-point lintas frame drag.
pub fn translate_shape(shape: &KernelShape, dx: f64, dy: f64, dz: f64) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    cloned.set_global_translation(dvec3(dx, dy, dz));
    Ok(KernelShape(cloned))
}

/// Putar shape mengelilingi sumbu yang melewati titik `pivot` dengan arah `axis` sebesar `angle_rad` radian.
pub fn rotate_shape(
    shape: &KernelShape,
    pivot: (f64, f64, f64),
    axis: (f64, f64, f64),
    angle_rad: f64,
) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    cloned.rotate(
        dvec3(pivot.0, pivot.1, pivot.2),
        dvec3(axis.0, axis.1, axis.2),
        angle_rad,
    );
    Ok(KernelShape(cloned))
}

/// Scale shape secara UNIFORM (satu faktor sama utk X/Y/Z) mengelilingi `pivot` —
/// dipakai gizmo/panel resize body 3D. `factor` 1.0 = tanpa perubahan, 2.0 = 2x lebih
/// besar, 0.5 = separuh. Hanya uniform: lihat catatan `Shape::scale` di
/// `vendor/opencascade-0.2.0/src/primitives/shape.rs` (Perubahan #10) kenapa scale
/// per-sumbu berbeda (non-uniform) belum didukung binding OCCT versi ini.
pub fn scale_shape(shape: &KernelShape, pivot: (f64, f64, f64), factor: f64) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    cloned.scale(dvec3(pivot.0, pivot.1, pivot.2), factor);
    Ok(KernelShape(cloned))
}

/// Transformasi shape dengan pergeseran (dx, dy, dz) dan rotasi sekeliling sumbu `axis` di `pivot`.
pub fn transform_shape(
    shape: &KernelShape,
    translation: (f64, f64, f64),
    pivot: (f64, f64, f64),
    axis: (f64, f64, f64),
    angle_rad: f64,
) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    if angle_rad.abs() > 1e-6 {
        cloned.rotate(
            dvec3(pivot.0, pivot.1, pivot.2),
            dvec3(axis.0, axis.1, axis.2),
            angle_rad,
        );
    }
    if translation.0.abs() > 1e-6 || translation.1.abs() > 1e-6 || translation.2.abs() > 1e-6 {
        cloned.set_global_translation(dvec3(translation.0, translation.1, translation.2));
    }
    Ok(KernelShape(cloned))
}

/// Gabungkan beberapa `KernelShape` menjadi 1 B-Rep Shape tunggal (TopoDS_Compound).
/// Menghasilkan 1 objek solid gabungan terpadu yang dapat di-tessellate, di-export, dan dimanipulasi utuh.
pub fn make_compound(shapes: &[&KernelShape]) -> Result<KernelShape> {
    if shapes.is_empty() {
        anyhow::bail!("tidak ada shape untuk digabungkan menjadi compound");
    }
    if shapes.len() == 1 {
        return clone_shape(shapes[0]);
    }
    let _guard = lock_kernel();
    let refs: Vec<&Shape> = shapes.iter().map(|s| s.inner()).collect();
    let compound = opencascade::primitives::Compound::from_shapes(refs);
    let combined: Shape = compound.into();
    Ok(KernelShape::from_inner(combined))
}

/// Ekstraksi segmen garis tepi 3D (B-Rep model curves / mesh feature edges) dari `KernelShape`.
/// Digunakan untuk rendering garis tepi solid CAD ("Shaded with Visible Edges") di viewport 3D.
pub fn extract_shape_edges(
    shape: &KernelShape,
    mesh: Option<&KernelMesh>,
) -> Vec<([f32; 3], [f32; 3])> {
    let _guard = lock_kernel();
    let mut raw_segments = Vec::new();
    let occ_shape = shape.inner();

    let mut seen_pairs = std::collections::HashSet::new();
    let quantize = |p: [f32; 3]| -> (i64, i64, i64) {
        (
            (p[0] * 100.0).round() as i64,
            (p[1] * 100.0).round() as i64,
            (p[2] * 100.0).round() as i64,
        )
    };

    const MAX_EDGES: usize = 5000;
    for (idx, edge) in occ_shape.edges().enumerate() {
        if idx >= MAX_EDGES {
            break;
        }
        let approx = edge.approximation_segments();
        let points: Vec<[f32; 3]> = approx
            .map(|p| [p.x as f32, p.y as f32, p.z as f32])
            .collect();

        if points.len() >= 2 {
            for w in points.windows(2) {
                let p1 = w[0];
                let p2 = w[1];
                let dx = p1[0] - p2[0];
                let dy = p1[1] - p2[1];
                let dz = p1[2] - p2[2];
                if dx * dx + dy * dy + dz * dz > 1e-6 {
                    let q1 = quantize(p1);
                    let q2 = quantize(p2);
                    let key = if q1 <= q2 { (q1, q2) } else { (q2, q1) };
                    if seen_pairs.insert(key) {
                        raw_segments.push((p1, p2));
                    }
                }
            }
        } else {
            let p1 = edge.start_point();
            let p2 = edge.end_point();
            let v1 = [p1.x as f32, p1.y as f32, p1.z as f32];
            let v2 = [p2.x as f32, p2.y as f32, p2.z as f32];
            let dx = v1[0] - v2[0];
            let dy = v1[1] - v2[1];
            let dz = v1[2] - v2[2];
            if dx * dx + dy * dy + dz * dz > 1e-6 {
                let q1 = quantize(v1);
                let q2 = quantize(v2);
                let key = if q1 <= q2 { (q1, q2) } else { (q2, q1) };
                if seen_pairs.insert(key) {
                    raw_segments.push((v1, v2));
                }
            }
        }
    }

    // Jika tidak ada tepi B-Rep (misal pure mesh STL), fallback ke feature crease edges
    if raw_segments.is_empty() {
        if let Some(m) = mesh {
            let feat = crate::hlr::extract_mesh_feature_edges(m);
            for (p1, p2) in feat {
                raw_segments.push(([p1.x, p1.y, p1.z], [p2.x, p2.y, p2.z]));
            }
        }
    }

    raw_segments
}
