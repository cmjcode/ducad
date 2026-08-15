//! Format native `.cadraw` (Fase 5): satu file JSON versi-ed berisi seluruh
//! sketch (entitas+constraint) dan semua body 3D. Geometri B-rep tiap body
//! disematkan sebagai teks STEP (lewat `KernelShape::to_step_string`) —
//! satu-satunya cara persis menyalin B-rep lewat binding `opencascade-rs`
//! ini (lihat catatan `deep_clone` di `cadraw-kernel`), jadi dipakai ulang
//! di sini alih-alih menulis serializer B-rep sendiri.
//!
//! `Sketch` (dan `Entity`/`Constraint`/`PointRef` di dalamnya) di-derive
//! `Serialize`/`Deserialize` LANGSUNG di `cadraw-sketch` — bukan struct
//! salinan di sini — supaya cuma ada satu sumber kebenaran bentuk data.
//! `EntityId` di dalam `Constraint`/`PointRef` ikut ter-roundtrip APA
//! ADANYA berkat fitur "serde" `slotmap` (index+versi internal disimpan),
//! jadi tidak perlu remapping id manual.
//!
//! Body TIDAK menyimpan `BodyId` — beda dengan `EntityId` yang jadi
//! rujukan silang lewat constraint, tidak ada apa pun di file ini yang
//! merujuk `BodyId` lintas body, jadi body cukup direkonstruksi sebagai
//! daftar baru saat load (urutan dipertahankan, `BodyId` baru dibuat
//! pemanggil lewat `Document::add_body`).

use anyhow::{bail, Context, Result};
use cadraw_kernel::KernelShape;
use cadraw_sketch::Sketch;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Versi format berkas — dinaikkan tiap kali skema `CadrawFile` berubah
/// dengan cara yang tak-kompatibel-mundur. `load` menolak versi yang lebih
/// baru dari yang dikenal crate ini (lebih aman daripada mencoba baca &
/// diam-diam salah); versi LAMA dari yang dikenal saat ini masih diterima
/// (belum ada migrasi ditulis karena baru versi 1 yang pernah ada).
pub const FORMAT_VERSION: u32 = 1;

/// Satu body 3D dalam file native — nama, visibilitas, dan geometri B-rep
/// lengkap sebagai teks STEP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeBody {
    pub name: String,
    pub visible: bool,
    /// Teks STEP AP214 lengkap (bukan mesh) — lihat catatan modul.
    pub step: String,
}

/// Isi lengkap satu dokumen CADRAW, siap ditulis/dibaca sebagai JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadrawFile {
    pub format_version: u32,
    pub sketch: Sketch,
    #[serde(default)]
    pub front_sketch: Option<Sketch>,
    #[serde(default)]
    pub right_sketch: Option<Sketch>,
    pub bodies: Vec<NativeBody>,
}

/// Body yang SUDAH dimuat (geometrinya sudah direkonstruksi jadi
/// `KernelShape` sungguhan) — hasil `load`, siap dipasang pemanggil ke
/// `ModelDoc`-nya sendiri (lewat `Document::add_body` + insert geometri).
pub struct LoadedBody {
    pub name: String,
    pub visible: bool,
    pub shape: KernelShape,
}

/// Hasil `load`: sketch lengkap dari ketiga bidang (Top, Front, Right) + semua body dengan geometri kernel hidup.
pub struct LoadedDocument {
    pub sketch: Sketch,
    pub front_sketch: Sketch,
    pub right_sketch: Sketch,
    pub bodies: Vec<LoadedBody>,
}

impl LoadedDocument {
    /// Mengembalikan seluruh sketch per bidang sebagai array `[Top, Front, Right]`.
    pub fn into_sketches(self) -> [Sketch; 3] {
        [self.sketch, self.front_sketch, self.right_sketch]
    }
}

/// Simpan dokumen multi-bidang (Top, Front, Right) ke `path` sebagai JSON.
pub fn save_multi_plane(
    path: impl AsRef<Path>,
    sketches: &[Sketch; 3],
    bodies: &[(&str, bool, &KernelShape)],
) -> Result<()> {
    let bodies = bodies
        .iter()
        .map(|(name, visible, shape)| {
            Ok(NativeBody {
                name: name.to_string(),
                visible: *visible,
                step: shape
                    .to_step_string()
                    .with_context(|| format!("gagal serialize body '{name}' ke STEP"))?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let file = CadrawFile {
        format_version: FORMAT_VERSION,
        sketch: sketches[0].clone(),
        front_sketch: Some(sketches[1].clone()),
        right_sketch: Some(sketches[2].clone()),
        bodies,
    };
    let json = serde_json::to_string_pretty(&file).context("gagal serialize dokumen ke JSON")?;
    std::fs::write(path, json).context("gagal menulis file .cadraw")?;
    Ok(())
}

/// Simpan dokumen (single sketch Top XY) ke `path` sebagai JSON.
pub fn save(path: impl AsRef<Path>, sketch: &Sketch, bodies: &[(&str, bool, &KernelShape)]) -> Result<()> {
    save_multi_plane(
        path,
        &[sketch.clone(), Sketch::default(), Sketch::default()],
        bodies,
    )
}

/// Muat dokumen dari `path`. Menolak `format_version` yang lebih baru dari
/// `FORMAT_VERSION` yang dikenal build ini (lihat catatan konstanta).
pub fn load(path: impl AsRef<Path>) -> Result<LoadedDocument> {
    let json = std::fs::read_to_string(&path).context("gagal membaca file .cadraw")?;
    let file: CadrawFile =
        serde_json::from_str(&json).context("gagal parse file .cadraw (format tidak dikenal atau rusak)")?;
    if file.format_version > FORMAT_VERSION {
        bail!(
            "file .cadraw ini dibuat versi format {} — build CADRAW ini cuma mengenal sampai versi {FORMAT_VERSION}, perbarui aplikasi",
            file.format_version
        );
    }

    let bodies = file
        .bodies
        .into_iter()
        .map(|b| {
            let shape = KernelShape::from_step_string(&b.step)
                .with_context(|| format!("gagal baca geometri body '{}' dari STEP tersimpan", b.name))?;
            Ok(LoadedBody {
                name: b.name,
                visible: b.visible,
                shape,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let front_sketch = file.front_sketch.unwrap_or_default();
    let right_sketch = file.right_sketch.unwrap_or_default();

    Ok(LoadedDocument {
        sketch: file.sketch,
        front_sketch,
        right_sketch,
        bodies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::occt_test_lock::LOCK as TEST_LOCK;
    use cadraw_kernel::{extrude_profile, Profile, ProfileSegment};
    use cadraw_sketch::Entity;
    use glam::DVec2;

    fn rect_profile(w: f64, h: f64) -> Profile {
        Profile::Loop(vec![
            ProfileSegment::Line { start: (0.0, 0.0), end: (w, 0.0) },
            ProfileSegment::Line { start: (w, 0.0), end: (w, h) },
            ProfileSegment::Line { start: (w, h), end: (0.0, h) },
            ProfileSegment::Line { start: (0.0, h), end: (0.0, 0.0) },
        ])
    }

    fn temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "cadraw-io-test-{tag}-{}-{:?}.cadraw",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn save_load_roundtrip_preserves_sketch_and_body() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut sketch = Sketch::default();
        let line_id = sketch.entities.insert(Entity::Line {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(10.0, 0.0),
        });
        sketch
            .constraints
            .push(cadraw_sketch::constraint::Constraint::Horizontal { line: line_id });

        let shape = extrude_profile(&rect_profile(20.0, 10.0), 5.0).unwrap();
        let path = temp_path("roundtrip");

        save(&path, &sketch, &[("Body 1", true, &shape)]).unwrap();
        let loaded = load(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.sketch.entities.len(), 1);
        assert_eq!(loaded.sketch.constraints.len(), 1);
        assert!(
            loaded.sketch.entities.contains_key(line_id),
            "EntityId harus roundtrip persis lewat slotmap serde"
        );
        assert_eq!(loaded.bodies.len(), 1);
        assert_eq!(loaded.bodies[0].name, "Body 1");
        assert!(loaded.bodies[0].visible);
        assert_eq!(
            loaded.bodies[0].shape.tessellate().positions.len(),
            shape.tessellate().positions.len()
        );
    }

    #[test]
    fn save_load_roundtrip_empty_document() {
        let _guard = TEST_LOCK.lock().unwrap();
        let sketch = Sketch::default();
        let path = temp_path("empty");
        save(&path, &sketch, &[]).unwrap();
        let loaded = load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.sketch.entities.len(), 0);
        assert_eq!(loaded.bodies.len(), 0);
    }

    #[test]
    fn load_rejects_future_format_version() {
        let _guard = TEST_LOCK.lock().unwrap();
        let sketch = Sketch::default();
        let path = temp_path("future");
        save(&path, &sketch, &[]).unwrap();
        // Tempel angka versi lewat text-replace di atas dokumen VALID hasil
        // `save` sendiri — bukan JSON tulis-tangan — supaya test ini murni
        // menguji penolakan versi, bukan kebetulan gagal karena bentuk
        // internal `SlotMap` (yang selalu punya slot sentinel index 0, jadi
        // `"entities": []` mentah bukan representasi valid).
        let json = std::fs::read_to_string(&path).unwrap();
        let bumped = json.replacen("\"format_version\": 1", "\"format_version\": 999999", 1);
        assert_ne!(json, bumped, "replace format_version harus benar-benar kena");
        std::fs::write(&path, bumped).unwrap();

        let result = load(&path);
        let _ = std::fs::remove_file(&path);
        let err = match result {
            Ok(_) => panic!("load harus menolak format_version dari masa depan"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("versi format"));
    }

    #[test]
    fn load_rejects_garbage() {
        let path = temp_path("garbage");
        std::fs::write(&path, "bukan json sama sekali").unwrap();
        let err = load(&path);
        let _ = std::fs::remove_file(&path);
        assert!(err.is_err());
    }

    #[test]
    fn save_load_multi_plane_roundtrip() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut top = Sketch::default();
        top.entities.insert(Entity::Circle { center: DVec2::ZERO, radius: 10.0 });
        let mut front = Sketch::default();
        front.entities.insert(Entity::Line { start: DVec2::ZERO, end: DVec2::new(10.0, 20.0) });
        let mut right = Sketch::default();
        right.entities.insert(Entity::Circle { center: DVec2::new(5.0, 5.0), radius: 3.0 });

        let sketches = [top, front, right];
        let path = temp_path("multi_plane");
        save_multi_plane(&path, &sketches, &[]).unwrap();

        let loaded = load(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.sketch.entities.len(), 1);
        assert_eq!(loaded.front_sketch.entities.len(), 1);
        assert_eq!(loaded.right_sketch.entities.len(), 1);

        let array = loaded.into_sketches();
        assert_eq!(array[0].entities.len(), 1);
        assert_eq!(array[1].entities.len(), 1);
        assert_eq!(array[2].entities.len(), 1);
    }
}
