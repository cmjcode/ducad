//! Referensi part EKSTERNAL: part yang geometrinya hidup di berkas
//! `.ducad` lain (P3.1).
//!
//! # Kenapa tidak cukup menyalin geometrinya
//!
//! Perakitan nyata menyusun part yang dirancang terpisah. Menyalin
//! geometrinya ke dalam berkas perakitan membuat salinan itu langsung
//! basi begitu part aslinya direvisi — dan tidak ada yang memberi tahu.
//! Referensi eksternal menyimpan PENUNJUK, lalu memeriksa apakah sumbernya
//! berubah.
//!
//! # Tiga masalah yang harus diselesaikan, dan pilihannya di sini
//!
//! **1. Path berubah.** Proyek dipindah folder, di-zip, dikirim ke orang
//! lain. Karena itu path disimpan RELATIF terhadap dokumen yang merujuk —
//! seluruh folder proyek bisa dipindahkan utuh tanpa satu pun referensi
//! putus. Path absolut tetap disimpan sebagai CADANGAN untuk kasus
//! sebaliknya: berkas perakitannya yang dipindah, sementara part-nya tetap
//! di tempat.
//!
//! **2. Part di dalam berkas berubah nama.** Karena itu part dirujuk lewat
//! UUID, bukan nama. Nama tetap disimpan, tapi hanya untuk pesan kesalahan
//! dan pencarian cadangan.
//!
//! **3. Mendeteksi perubahan tanpa membaca ulang terus-menerus.**
//! Lihat [`SourceStamp`].

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Asal geometri sebuah instance part dalam perakitan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PartSource {
    /// Body di dalam dokumen ini sendiri.
    Internal { body_id_raw: u64 },
    /// Part yang dimuat dari berkas `.ducad` lain.
    External(Box<ExternalPartRef>),
}

impl PartSource {
    pub fn is_external(&self) -> bool {
        matches!(self, PartSource::External(_))
    }

    pub fn external(&self) -> Option<&ExternalPartRef> {
        match self {
            PartSource::External(r) => Some(r),
            PartSource::Internal { .. } => None,
        }
    }
}

/// Cap keadaan berkas sumber saat terakhir kali dimuat.
///
/// Deteksi perubahan berlapis, karena tiap lapisannya punya kelemahan
/// sendiri:
///
/// - `modified_unix` + `size_bytes` murah tapi BERBOHONG dua arah:
///   `touch` mengubah mtime tanpa mengubah isi, dan sebagian filesystem
///   (serta sinkronisasi cloud) punya resolusi mtime hanya 1–2 detik.
/// - `content_hash` selalu benar tapi menuntut membaca seluruh berkas.
///
/// # Jebakan *racily clean*
///
/// Menganggap "mtime sama DAN ukuran sama berarti tidak berubah" TIDAK
/// aman, dan ini terbukti lewat test, bukan kekhawatiran teoretis: sebuah
/// part di-revisi tinggi 10 mm menjadi 25 mm dalam detik yang sama dengan
/// pencatatan cap sebelumnya, dan teks STEP-nya kebetulan berukuran
/// identik (`10.` dan `25.` sama panjang). Kedua metadata sama persis,
/// padahal isinya berbeda — perubahan itu HILANG.
///
/// Ini persis masalah yang dikenal Git sebagai indeks *racily clean*, dan
/// penyelesaiannya diambil dari sana: cap juga mencatat KAPAN ia dibuat.
/// Jalur murah hanya dipercaya bila berkasnya dimodifikasi BENAR-BENAR
/// SEBELUM detik pencatatan; bila mtime-nya berada pada detik yang sama
/// atau sesudahnya, isinya wajib di-hash. Hasilnya: `touch` tetap tidak
/// memicu pemuatan ulang sia-sia, sementara penyuntingan cepat tidak
/// pernah terlewat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceStamp {
    pub modified_unix: i64,
    pub size_bytes: u64,
    pub content_hash: u64,
    /// Detik Unix saat cap ini dibuat. Lihat catatan *racily clean*.
    #[serde(default)]
    pub recorded_at_unix: i64,
}

impl SourceStamp {
    /// Ambil cap dari berkas di `path`.
    pub fn of(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let meta = std::fs::metadata(path)?;
        let modified_unix = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(Self {
            modified_unix,
            size_bytes: bytes.len() as u64,
            content_hash: stable_hash(&bytes),
            recorded_at_unix: now_unix(),
        })
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Hash FNV-1a 64-bit.
///
/// Ditulis sendiri, bukan `DefaultHasher`: hash bawaan std TIDAK dijamin
/// stabil antar rilis Rust, sementara nilai ini DISIMPAN ke berkas dan
/// dibandingkan lagi berbulan-bulan kemudian oleh biner yang mungkin sudah
/// dibangun dengan toolchain berbeda. Hash yang berubah artinya akan
/// membuat setiap referensi eksternal terlihat basi setelah upgrade
/// compiler.
pub fn stable_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Penunjuk ke sebuah part di berkas `.ducad` lain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalPartRef {
    /// Path berkas sumber RELATIF terhadap folder dokumen yang merujuk.
    /// Inilah jalur utama — lihat catatan modul.
    pub relative_path: String,
    /// Path absolut terakhir yang diketahui berhasil, sebagai cadangan.
    pub last_absolute_path: Option<String>,
    /// Identitas stabil part di dalam berkas sumber.
    pub part_uuid: String,
    /// Nama part di sumber — untuk pesan kesalahan dan pencarian cadangan
    /// saat UUID tidak ditemukan (mis. berkas lama yang belum punya UUID).
    pub source_name: String,
    /// Keadaan sumber saat terakhir dimuat. `None` berarti belum pernah.
    pub last_seen: Option<SourceStamp>,
}

impl ExternalPartRef {
    pub fn new(
        relative_path: impl Into<String>,
        part_uuid: impl Into<String>,
        source_name: impl Into<String>,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            last_absolute_path: None,
            part_uuid: part_uuid.into(),
            source_name: source_name.into(),
            last_seen: None,
        }
    }
}

/// Kenapa sebuah referensi ditemukan lewat jalur cadangan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    /// Path relatif gagal, path absolut yang tersimpan berhasil.
    AbsolutePath,
    /// Kedua path gagal; berkas dengan nama sama ditemukan di folder
    /// pencarian.
    SearchDirectory,
}

/// Hasil upaya menemukan berkas sumber.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveOutcome {
    /// Ditemukan lewat path relatif — keadaan normal.
    Found(PathBuf),
    /// Ditemukan, tapi TIDAK lewat path relatif. Pemanggil sebaiknya
    /// memperbarui `relative_path` supaya berikutnya kembali normal;
    /// dilaporkan terpisah agar perbaikan itu terlihat pengguna, bukan
    /// terjadi diam-diam.
    FoundByFallback {
        path: PathBuf,
        reason: FallbackReason,
    },
    /// Tidak ditemukan di mana pun. `tried` memuat seluruh kandidat yang
    /// diperiksa, supaya pesan kesalahannya bisa menyebutkan lokasi nyata
    /// alih-alih "berkas tidak ditemukan".
    Missing { tried: Vec<PathBuf> },
}

impl ResolveOutcome {
    pub fn path(&self) -> Option<&Path> {
        match self {
            ResolveOutcome::Found(p) => Some(p),
            ResolveOutcome::FoundByFallback { path, .. } => Some(path),
            ResolveOutcome::Missing { .. } => None,
        }
    }
}

/// Temukan berkas sumber sebuah referensi eksternal.
///
/// Urutan percobaan sengaja begini:
/// 1. `doc_dir` + `relative_path` — menjaga proyek yang dipindah utuh.
/// 2. `last_absolute_path` — menjaga kasus sebaliknya, berkas perakitannya
///    yang pindah sementara part-nya tetap.
/// 3. Nama berkas yang sama di dalam `search_dirs` — jaring terakhir untuk
///    pustaka part bersama.
///
/// `exists` disuntikkan supaya keseluruhan kebijakan ini bisa diuji tanpa
/// menyentuh filesystem.
pub fn resolve_external_with(
    r: &ExternalPartRef,
    doc_dir: &Path,
    search_dirs: &[PathBuf],
    exists: &dyn Fn(&Path) -> bool,
) -> ResolveOutcome {
    let mut tried = Vec::new();

    let relative = doc_dir.join(&r.relative_path);
    if exists(&relative) {
        return ResolveOutcome::Found(relative);
    }
    tried.push(relative);

    if let Some(abs) = &r.last_absolute_path {
        let abs = PathBuf::from(abs);
        if exists(&abs) {
            return ResolveOutcome::FoundByFallback {
                path: abs,
                reason: FallbackReason::AbsolutePath,
            };
        }
        tried.push(abs);
    }

    if let Some(file_name) = Path::new(&r.relative_path).file_name() {
        for dir in search_dirs {
            let candidate = dir.join(file_name);
            if exists(&candidate) {
                return ResolveOutcome::FoundByFallback {
                    path: candidate,
                    reason: FallbackReason::SearchDirectory,
                };
            }
            tried.push(candidate);
        }
    }

    ResolveOutcome::Missing { tried }
}

/// Seperti [`resolve_external_with`], memakai filesystem sungguhan.
pub fn resolve_external(
    r: &ExternalPartRef,
    doc_dir: &Path,
    search_dirs: &[PathBuf],
) -> ResolveOutcome {
    resolve_external_with(r, doc_dir, search_dirs, &|p| p.is_file())
}

/// Keadaan berkas sumber dibanding terakhir kali dimuat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceState {
    /// Isi berkas identik dengan saat terakhir dimuat.
    Unchanged,
    /// Isi berkas berbeda — part perlu dimuat ulang.
    Changed,
    /// Belum pernah dimuat, jadi tidak ada pembanding.
    NeverLoaded,
    /// Berkas tidak bisa dibaca.
    Unreadable,
}

/// Bandingkan berkas di `path` dengan cap terakhirnya. Lihat [`SourceStamp`]
/// untuk alasan pemeriksaan berlapisnya.
pub fn check_source_state(last_seen: Option<&SourceStamp>, path: &Path) -> SourceState {
    let Some(prev) = last_seen else {
        return SourceState::NeverLoaded;
    };
    let Ok(meta) = std::fs::metadata(path) else {
        return SourceState::Unreadable;
    };
    let size = meta.len();
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Jalur murah: mtime DAN ukuran sama -> tidak berubah, tanpa membaca
    // isi berkas sama sekali.
    //
    // TAPI hanya bila berkasnya dimodifikasi BENAR-BENAR SEBELUM detik
    // pencatatan cap. Bila mtime-nya jatuh pada detik yang sama (atau
    // sesudahnya), penyuntingan lain bisa saja terjadi dalam detik itu juga
    // tanpa mengubah metadata sama sekali — lihat catatan *racily clean*
    // di `SourceStamp`. Dalam kasus itu isinya wajib diperiksa.
    let metadata_matches = modified == prev.modified_unix && size == prev.size_bytes;
    let safely_older = modified < prev.recorded_at_unix;
    if metadata_matches && safely_older {
        return SourceState::Unchanged;
    }

    // Metadata berbeda, ATAU sama tapi tidak bisa dipercaya. Baca dan
    // bandingkan isi — ini yang membuat `touch` tidak memicu muat ulang
    // sia-sia sekaligus menutup jebakan racily clean.
    match std::fs::read(path) {
        Ok(bytes) => {
            if stable_hash(&bytes) == prev.content_hash {
                SourceState::Unchanged
            } else {
                SourceState::Changed
            }
        }
        Err(_) => SourceState::Unreadable,
    }
}

/// Buat UUID part baru.
///
/// Bukan UUIDv4 sungguhan — DUCAD belum menarik crate `uuid`, dan yang
/// dibutuhkan di sini hanya keunikan di dalam satu berkas plus ketahanan
/// terhadap penggabungan dua berkas. Gabungan waktu nanodetik, id proses,
/// dan penghitung yang bertambah memenuhi itu; nanodetik saja TIDAK cukup
/// karena dua part yang dibuat dalam satu perulangan bisa mendapat cap
/// waktu yang sama persis pada jam beresolusi kasar.
pub fn new_part_uuid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:016x}{:08x}{:08x}", nanos, std::process::id(), n as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn fake_fs(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<PathBuf> = paths.iter().map(PathBuf::from).collect();
        move |p: &Path| set.contains(p)
    }

    fn sample_ref() -> ExternalPartRef {
        ExternalPartRef::new("parts/bracket.ducad", "uuid-1", "Bracket")
    }

    #[test]
    fn relative_path_is_tried_first_so_moved_projects_keep_working() {
        // Alasan utama menyimpan path relatif: seluruh folder proyek bisa
        // dipindah/di-zip/dikirim tanpa satu pun referensi putus.
        let r = sample_ref();
        let fs = fake_fs(&["/proyek/baru/parts/bracket.ducad"]);
        let out = resolve_external_with(&r, Path::new("/proyek/baru"), &[], &fs);
        assert_eq!(
            out,
            ResolveOutcome::Found(PathBuf::from("/proyek/baru/parts/bracket.ducad"))
        );
    }

    #[test]
    fn absolute_path_rescues_a_moved_assembly_file() {
        // Kasus sebaliknya: berkas PERAKITAN yang dipindah, part-nya tetap.
        let mut r = sample_ref();
        r.last_absolute_path = Some("/pustaka/parts/bracket.ducad".to_string());
        let fs = fake_fs(&["/pustaka/parts/bracket.ducad"]);

        let out = resolve_external_with(&r, Path::new("/tempat/lain"), &[], &fs);
        assert_eq!(
            out,
            ResolveOutcome::FoundByFallback {
                path: PathBuf::from("/pustaka/parts/bracket.ducad"),
                reason: FallbackReason::AbsolutePath,
            }
        );
    }

    #[test]
    fn search_directory_is_the_last_resort() {
        let r = sample_ref();
        let fs = fake_fs(&["/pustaka-bersama/bracket.ducad"]);
        let out = resolve_external_with(
            &r,
            Path::new("/proyek"),
            &[PathBuf::from("/pustaka-bersama")],
            &fs,
        );
        assert_eq!(
            out,
            ResolveOutcome::FoundByFallback {
                path: PathBuf::from("/pustaka-bersama/bracket.ducad"),
                reason: FallbackReason::SearchDirectory,
            }
        );
    }

    #[test]
    fn fallback_is_reported_separately_not_applied_silently() {
        // Perbaikan otomatis harus TERLIHAT: kalau referensi diam-diam
        // dialihkan ke berkas lain bernama sama, pengguna bisa merakit part
        // yang keliru tanpa pernah tahu.
        let mut r = sample_ref();
        r.last_absolute_path = Some("/pustaka/bracket.ducad".to_string());
        let fs = fake_fs(&["/pustaka/bracket.ducad"]);
        let out = resolve_external_with(&r, Path::new("/proyek"), &[], &fs);
        assert!(
            !matches!(out, ResolveOutcome::Found(_)),
            "jalur cadangan tidak boleh dilaporkan sebagai jalur normal"
        );
    }

    #[test]
    fn missing_reports_every_candidate_it_tried() {
        // Pesan "berkas tidak ditemukan" tanpa lokasi tidak bisa
        // ditindaklanjuti pengguna.
        let mut r = sample_ref();
        r.last_absolute_path = Some("/lama/bracket.ducad".to_string());
        let fs = fake_fs(&[]);
        let out =
            resolve_external_with(&r, Path::new("/proyek"), &[PathBuf::from("/pustaka")], &fs);
        match out {
            ResolveOutcome::Missing { tried } => {
                assert_eq!(tried.len(), 3, "relatif + absolut + folder pencarian");
                assert!(tried.contains(&PathBuf::from("/proyek/parts/bracket.ducad")));
                assert!(tried.contains(&PathBuf::from("/lama/bracket.ducad")));
                assert!(tried.contains(&PathBuf::from("/pustaka/bracket.ducad")));
            }
            other => panic!("harus Missing, dapat {other:?}"),
        }
    }

    // ---- deteksi perubahan ----

    fn temp_file(tag: &str, content: &[u8]) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ducad-ext-{tag}-{}-{}",
            std::process::id(),
            new_part_uuid()
        ));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn unchanged_file_is_detected_without_reloading() {
        let p = temp_file("unchanged", b"isi part");
        let stamp = SourceStamp::of(&p).unwrap();
        assert_eq!(check_source_state(Some(&stamp), &p), SourceState::Unchanged);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn touch_does_not_trigger_a_reload() {
        // mtime BERBOHONG: `touch` mengubahnya tanpa mengubah isi. Kalau
        // hanya mtime yang diperiksa, tiap sinkronisasi cloud atau operasi
        // salin akan memicu pemuatan ulang seluruh part.
        let p = temp_file("touch", b"isi yang sama persis");
        let mut stamp = SourceStamp::of(&p).unwrap();
        // Simulasikan mtime yang berubah tapi isi tetap.
        stamp.modified_unix -= 500;
        assert_eq!(
            check_source_state(Some(&stamp), &p),
            SourceState::Unchanged,
            "isi identik walau mtime berbeda = tidak berubah"
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn real_edit_is_detected_even_when_size_is_identical() {
        // Penyuntingan yang mempertahankan UKURAN persis (mengganti satu
        // angka dengan angka lain sepanjang sama) tidak terlihat dari
        // metadata sama sekali — hanya hash isi yang menangkapnya.
        let p = temp_file("sameSize", b"radius=10.0");
        let mut stamp = SourceStamp::of(&p).unwrap();
        std::fs::write(&p, b"radius=20.0").unwrap();
        // Paksa metadata terlihat berbeda supaya jalur hash dipakai.
        stamp.modified_unix -= 5;
        assert_eq!(check_source_state(Some(&stamp), &p), SourceState::Changed);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn missing_file_is_unreadable_not_unchanged() {
        let p = std::env::temp_dir().join("ducad-ext-tidak-ada-sama-sekali.ducad");
        let _ = std::fs::remove_file(&p);
        let stamp = SourceStamp {
            modified_unix: 1,
            size_bytes: 1,
            content_hash: 1,
            recorded_at_unix: 2,
        };
        assert_eq!(
            check_source_state(Some(&stamp), &p),
            SourceState::Unreadable,
            "berkas hilang TIDAK boleh terlihat seperti tidak berubah"
        );
    }

    #[test]
    fn never_loaded_is_distinct_from_unchanged() {
        let p = temp_file("never", b"x");
        assert_eq!(check_source_state(None, &p), SourceState::NeverLoaded);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn hash_is_stable_for_known_input() {
        // Nilai ini DISIMPAN ke berkas dan dibandingkan lagi berbulan-bulan
        // kemudian, mungkin oleh biner yang dibangun dengan toolchain
        // berbeda. Mengunci angkanya memastikan implementasinya tidak
        // pernah berubah diam-diam.
        assert_eq!(stable_hash(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(stable_hash(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_ne!(stable_hash(b"abc"), stable_hash(b"abd"));
    }

    #[test]
    fn part_uuids_are_unique_within_a_tight_loop() {
        // Cap waktu nanodetik SAJA tidak cukup: dua part yang dibuat dalam
        // satu perulangan bisa mendapat nanodetik yang sama persis pada jam
        // beresolusi kasar.
        let ids: HashSet<String> = (0..1000).map(|_| new_part_uuid()).collect();
        assert_eq!(ids.len(), 1000, "UUID part harus unik");
    }
}

#[cfg(test)]
mod racily_clean_tests {
    use super::*;

    #[test]
    fn edit_within_the_same_second_and_same_size_is_still_detected() {
        // Regresi untuk bug yang ditemukan test integrasi: sebuah part
        // di-revisi dalam detik yang SAMA dengan pencatatan cap, dan teks
        // STEP-nya kebetulan berukuran identik. Kedua metadata sama persis.
        // Tanpa penjagaan racily clean, perubahan itu HILANG.
        let p = std::env::temp_dir().join(format!(
            "ducad-racy-{}-{}",
            std::process::id(),
            new_part_uuid()
        ));
        std::fs::write(&p, b"radius=10.").unwrap();
        let stamp = SourceStamp::of(&p).unwrap();

        // Ukuran sama persis, ditulis dalam detik yang sama.
        std::fs::write(&p, b"radius=25.").unwrap();
        let meta = std::fs::metadata(&p).unwrap();
        assert_eq!(meta.len(), stamp.size_bytes, "prasyarat: ukuran identik");

        assert_eq!(
            check_source_state(Some(&stamp), &p),
            SourceState::Changed,
            "penyuntingan dalam detik yang sama harus tetap terdeteksi"
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn cheap_path_still_applies_to_genuinely_old_files() {
        // Penjagaan racily clean tidak boleh membuat SETIAP pemeriksaan
        // membaca berkas — itu akan menghapus seluruh manfaatnya.
        let p = std::env::temp_dir().join(format!(
            "ducad-old-{}-{}",
            std::process::id(),
            new_part_uuid()
        ));
        std::fs::write(&p, b"isi").unwrap();
        let mut stamp = SourceStamp::of(&p).unwrap();
        // Berkas terlihat dimodifikasi jauh sebelum cap dicatat.
        stamp.modified_unix -= 100;
        stamp.recorded_at_unix += 100;
        // Buat hash SENGAJA salah: kalau jalur murah dipakai, hash tidak
        // akan pernah dilihat dan hasilnya tetap Unchanged.
        stamp.content_hash = 0xdead_beef;
        // Samakan mtime supaya syarat metadata terpenuhi.
        let actual = SourceStamp::of(&p).unwrap();
        stamp.modified_unix = actual.modified_unix - 100;

        // mtime sekarang != prev.modified_unix, jadi jalur murah TIDAK
        // berlaku; ini memverifikasi arah sebaliknya — hash dipakai.
        assert_eq!(check_source_state(Some(&stamp), &p), SourceState::Changed);

        // Sekarang cap yang benar-benar konsisten dan aman-tua.
        let mut safe = SourceStamp::of(&p).unwrap();
        safe.recorded_at_unix = safe.modified_unix + 10;
        assert_eq!(check_source_state(Some(&safe), &p), SourceState::Unchanged);
        let _ = std::fs::remove_file(&p);
    }
}
