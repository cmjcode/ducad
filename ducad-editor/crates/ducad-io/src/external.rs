//! Pemuat part EKSTERNAL: mengambil satu body dari berkas `.ducad` lain
//! (P3.1).
//!
//! Kebijakan penemuan berkas dan deteksi perubahannya ada di
//! `ducad_core::external` — modul ini hanya menjalankannya lalu membaca
//! geometrinya.

use anyhow::{Context, Result};
use ducad_core::external::{
    check_source_state, ExternalPartRef, ResolveOutcome, SourceStamp, SourceState,
};
use ducad_kernel::KernelShape;
use std::path::{Path, PathBuf};

/// Bagaimana sebuah part ditemukan di dalam berkas sumbernya.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartMatch {
    /// Cocok lewat UUID — jalur normal, tahan terhadap penggantian nama.
    ByUuid,
    /// UUID tidak ditemukan, dicocokkan lewat nama.
    ///
    /// Ini terjadi pada berkas yang disimpan SEBELUM body punya UUID:
    /// `serde(default)` memberi UUID baru tiap kali berkas itu dimuat,
    /// jadi UUID yang tersimpan di referensi tidak akan pernah cocok.
    /// Pencocokan nama adalah satu-satunya jalan sampai berkas sumber
    /// disimpan ulang sekali.
    ByNameFallback,
}

/// Part eksternal yang berhasil dimuat.
pub struct LoadedExternalPart {
    pub shape: KernelShape,
    pub name: String,
    /// UUID yang sebenarnya ada di berkas sumber. Berbeda dari yang diminta
    /// bila pencocokan jatuh ke nama — pemanggil sebaiknya memperbaruinya
    /// supaya pemuatan berikutnya kembali lewat UUID.
    pub resolved_uuid: String,
    pub matched_by: PartMatch,
    /// Cap keadaan berkas saat dimuat, untuk perbandingan berikutnya.
    pub stamp: SourceStamp,
    /// Path berkas yang benar-benar dipakai.
    pub source_path: PathBuf,
}

/// Muat satu part dari berkas `.ducad` yang sudah diketahui lokasinya.
pub fn load_part_from_file(
    path: &Path,
    part_uuid: &str,
    fallback_name: &str,
) -> Result<LoadedExternalPart> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("gagal membaca berkas part '{}'", path.display()))?;
    let doc = crate::native::deserialize_raw(&json)
        .with_context(|| format!("berkas part '{}' tidak bisa dibaca", path.display()))?;

    let (body, matched_by) = match doc.bodies.iter().find(|b| b.uuid == part_uuid) {
        Some(b) => (b, PartMatch::ByUuid),
        None => {
            let by_name = doc
                .bodies
                .iter()
                .find(|b| b.name == fallback_name)
                .with_context(|| {
                    format!(
                        "part '{fallback_name}' (uuid {part_uuid}) tidak ada di '{}' — \
                         berkas sumber memuat {} body: {}",
                        path.display(),
                        doc.bodies.len(),
                        doc.bodies
                            .iter()
                            .map(|b| b.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;
            (by_name, PartMatch::ByNameFallback)
        }
    };

    let shape = KernelShape::from_step_string(&body.step)
        .with_context(|| format!("geometri part '{}' rusak", body.name))?;
    let stamp = SourceStamp::of(path)
        .with_context(|| format!("gagal mencatat cap berkas '{}'", path.display()))?;

    Ok(LoadedExternalPart {
        shape,
        name: body.name.clone(),
        resolved_uuid: body.uuid.clone(),
        matched_by,
        stamp,
        source_path: path.to_path_buf(),
    })
}

/// Temukan dan muat part eksternal.
///
/// `doc_dir` adalah folder dokumen yang MERUJUK — dasar bagi path relatif.
pub fn load_external_part(
    r: &ExternalPartRef,
    doc_dir: &Path,
    search_dirs: &[PathBuf],
) -> Result<(LoadedExternalPart, ResolveOutcome)> {
    let outcome = ducad_core::external::resolve_external(r, doc_dir, search_dirs);
    let Some(path) = outcome.path() else {
        let tried = match &outcome {
            ResolveOutcome::Missing { tried } => tried
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n  "),
            _ => String::new(),
        };
        anyhow::bail!(
            "berkas part '{}' tidak ditemukan. Lokasi yang dicoba:\n  {tried}",
            r.relative_path
        );
    };
    let loaded = load_part_from_file(path, &r.part_uuid, &r.source_name)?;
    Ok((loaded, outcome))
}

/// Periksa apakah sumber sebuah referensi berubah sejak terakhir dimuat,
/// TANPA memuat geometrinya.
///
/// Dipakai pemeriksaan berkala: memuat ulang seluruh part hanya untuk tahu
/// apakah ia berubah akan membuat perakitan besar tersendat.
pub fn poll_external_source(
    r: &ExternalPartRef,
    doc_dir: &Path,
    search_dirs: &[PathBuf],
) -> SourceState {
    let outcome = ducad_core::external::resolve_external(r, doc_dir, search_dirs);
    match outcome.path() {
        Some(p) => check_source_state(r.last_seen.as_ref(), p),
        None => SourceState::Unreadable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::{save_multi_plane_detailed, ExportBody};
    use crate::occt_test_lock::LOCK as TEST_LOCK;
    use ducad_kernel::{extrude_profile, Profile, ProfileSegment};
    use ducad_sketch::Sketch;

    fn rect(w: f64, h: f64) -> Profile {
        Profile::Loop(vec![
            ProfileSegment::Line { start: (0.0, 0.0), end: (w, 0.0) },
            ProfileSegment::Line { start: (w, 0.0), end: (w, h) },
            ProfileSegment::Line { start: (w, h), end: (0.0, h) },
            ProfileSegment::Line { start: (0.0, h), end: (0.0, 0.0) },
        ])
    }

    struct PartFile {
        dir: PathBuf,
        path: PathBuf,
    }

    impl Drop for PartFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Tulis berkas part berisi satu body bernama `name`.
    fn write_part(tag: &str, name: &str, height: f64) -> PartFile {
        let dir = std::env::temp_dir().join(format!(
            "ducad-extpart-{tag}-{}-{}",
            std::process::id(),
            ducad_core::new_part_uuid()
        ));
        std::fs::create_dir_all(dir.join("parts")).unwrap();
        let path = dir.join("parts").join("bracket.ducad");

        let shape = extrude_profile(&rect(20.0, 20.0), height).unwrap();
        let sketch = Sketch::default();
        let body = ExportBody {
            name,
            uuid: None,
            visible: true,
            material: ducad_core::Material::default(),
            shape: &shape,
            round_history: None,
        };
        save_multi_plane_detailed(&path, &[&sketch], &[body]).unwrap();
        PartFile { dir, path }
    }

    /// UUID body pertama di sebuah berkas part.
    fn uuid_in(path: &Path) -> String {
        let json = std::fs::read_to_string(path).unwrap();
        crate::native::deserialize_raw(&json).unwrap().bodies[0]
            .uuid
            .clone()
    }

    #[test]
    fn loads_a_part_from_another_file_by_uuid() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let f = write_part("basic", "Bracket", 10.0);
        let uuid = uuid_in(&f.path);

        let r = ExternalPartRef::new("parts/bracket.ducad", &uuid, "Bracket");
        let (loaded, outcome) = load_external_part(&r, &f.dir, &[]).unwrap();

        assert_eq!(loaded.matched_by, PartMatch::ByUuid);
        assert_eq!(loaded.name, "Bracket");
        assert!(matches!(outcome, ResolveOutcome::Found(_)));
        // Geometrinya benar-benar dimuat, bukan sekadar metadatanya.
        assert!((loaded.shape.volume().abs() - 20.0 * 20.0 * 10.0).abs() < 1e-6);
    }

    #[test]
    fn reference_survives_the_part_being_renamed() {
        // ALASAN memakai UUID dan bukan nama. Part di berkas sumber diganti
        // nama; referensinya harus tetap menemukannya.
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let f = write_part("rename", "Bracket", 10.0);
        let uuid = uuid_in(&f.path);

        // Tulis ulang berkas dengan nama berbeda tapi UUID yang SAMA.
        let json = std::fs::read_to_string(&f.path).unwrap();
        let mut doc = crate::native::deserialize_raw(&json).unwrap();
        doc.bodies[0].name = "Bracket REV-B".to_string();
        std::fs::write(&f.path, serde_json::to_string(&doc).unwrap()).unwrap();

        let r = ExternalPartRef::new("parts/bracket.ducad", &uuid, "Bracket");
        let (loaded, _) = load_external_part(&r, &f.dir, &[]).unwrap();
        assert_eq!(loaded.matched_by, PartMatch::ByUuid);
        assert_eq!(loaded.name, "Bracket REV-B", "harus mengikuti nama baru");
    }

    #[test]
    fn falls_back_to_name_when_uuid_is_unknown() {
        // Berkas yang disimpan sebelum body punya UUID: `serde(default)`
        // memberi UUID baru tiap dimuat, jadi UUID tersimpan tak akan pernah
        // cocok. Pencocokan nama adalah satu-satunya jalan.
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let f = write_part("fallback", "Bracket", 10.0);

        let r = ExternalPartRef::new("parts/bracket.ducad", "uuid-yang-tidak-ada", "Bracket");
        let (loaded, _) = load_external_part(&r, &f.dir, &[]).unwrap();
        assert_eq!(loaded.matched_by, PartMatch::ByNameFallback);
        // UUID sebenarnya dilaporkan balik supaya pemanggil bisa
        // memperbaikinya dan pemuatan berikutnya kembali lewat UUID.
        assert_ne!(loaded.resolved_uuid, "uuid-yang-tidak-ada");
    }

    #[test]
    fn missing_part_error_lists_what_the_file_actually_contains() {
        // Pesan "part tidak ditemukan" tanpa menyebut isi berkasnya tidak
        // bisa ditindaklanjuti pengguna.
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let f = write_part("missing", "Bracket", 10.0);

        let r = ExternalPartRef::new("parts/bracket.ducad", "tak-ada", "Nama Yang Salah");
        // `LoadedExternalPart` memuat `KernelShape` yang bukan `Debug`,
        // jadi `unwrap_err` tidak bisa dipakai di sini.
        let msg = match load_external_part(&r, &f.dir, &[]) {
            Err(e) => format!("{e:#}"),
            Ok(_) => panic!("seharusnya gagal"),
        };
        assert!(msg.contains("Bracket"), "pesan harus menyebut isi nyata: {msg}");
    }

    #[test]
    fn missing_file_error_lists_every_location_tried() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let r = ExternalPartRef::new("parts/hilang.ducad", "u", "X");
        let msg = match load_external_part(&r, Path::new("/tmp/ducad-tidak-ada"), &[]) {
            Err(e) => format!("{e:#}"),
            Ok(_) => panic!("seharusnya gagal"),
        };
        assert!(msg.contains("Lokasi yang dicoba"), "pesan: {msg}");
        assert!(msg.contains("parts/hilang.ducad"), "pesan: {msg}");
    }

    #[test]
    fn polling_detects_a_revised_source_without_loading_geometry() {
        // Dipakai pemeriksaan berkala: memuat ulang seluruh part hanya untuk
        // tahu apakah ia berubah akan membuat perakitan besar tersendat.
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let f = write_part("poll", "Bracket", 10.0);
        let uuid = uuid_in(&f.path);

        let mut r = ExternalPartRef::new("parts/bracket.ducad", &uuid, "Bracket");
        assert_eq!(
            poll_external_source(&r, &f.dir, &[]),
            SourceState::NeverLoaded
        );

        let (loaded, _) = load_external_part(&r, &f.dir, &[]).unwrap();
        r.last_seen = Some(loaded.stamp);
        assert_eq!(
            poll_external_source(&r, &f.dir, &[]),
            SourceState::Unchanged,
            "belum ada perubahan"
        );

        // Revisi part: tinggi 10 -> 25 mm.
        let revised = write_part("poll2", "Bracket", 25.0);
        std::fs::copy(&revised.path, &f.path).unwrap();
        assert_eq!(
            poll_external_source(&r, &f.dir, &[]),
            SourceState::Changed,
            "revisi sumber harus terdeteksi"
        );

        // Dan memuat ulang benar-benar memberi geometri yang baru.
        let (reloaded, _) = load_external_part(&r, &f.dir, &[]).unwrap();
        assert!(
            (reloaded.shape.volume().abs() - 20.0 * 20.0 * 25.0).abs() < 1e-6,
            "volume setelah muat ulang {}",
            reloaded.shape.volume()
        );
    }
}
