//! Logger aplikasi desktop: catatan `log::` ke stderr DAN ke berkas
//! `<data_dir>/logs/ducad.log` (`~/.ducad/logs/ducad.log`; iPad: Application
//! Support). Diambil dari praktik CADCraft (ADR 0009): aplikasi yang
//! diluncurkan dari Dock/Finder tidak punya terminal, jadi berkas inilah yang
//! bisa dilampirkan ke laporan bug — termasuk pesan panik, karena
//! [`install_panic_hook`] menulis setiap panik ke log sebelum hook bawaan
//! mencetaknya ke stderr.
//!
//! Setiap start memutar log sebelumnya (`ducad.log` → `ducad.1.log` →
//! `ducad.2.log`), sehingga log dari sesi yang crash selamat dari start
//! berikutnya. Berkas berhenti tumbuh di [`MAX_FILE_BYTES`] (peringatan yang
//! berulang tak boleh memenuhi disk). Filter: `RUST_LOG` gaya env_logger;
//! bawaan `warn` untuk semua crate dan `info` untuk crate `ducad*`. Berkas
//! yang gagal dibuat/ditulis hanya membuat stderr jadi satu-satunya tujuan —
//! logging tidak pernah menggagalkan start aplikasi.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Nama berkas log aktif di dalam [`log_dir`].
pub const LOG_FILE: &str = "ducad.log";
/// Berapa log sebelumnya yang disimpan (`ducad.1.log` … `ducad.<KEEP>.log`).
pub const KEEP: usize = 2;
/// Berkas log berhenti tumbuh setelah ukuran ini.
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// Filter bawaan bila `RUST_LOG` kosong.
pub const DEFAULT_FILTER: &str = "warn,ducad=info";

/// Folder log: `<data_dir>/logs`.
pub fn log_dir() -> PathBuf {
    crate::platform::data_dir().join("logs")
}

fn numbered(dir: &Path, n: usize) -> PathBuf {
    if n == 0 {
        dir.join(LOG_FILE)
    } else {
        dir.join(format!("ducad.{n}.log"))
    }
}

/// Geser log lama satu tingkat (`.log` → `.1.log` → … → `.<KEEP>.log`, yang
/// tertua dibuang) lalu buat berkas log kosong di `dir` (dibuat bila belum ada).
pub fn rotate(dir: &Path) -> std::io::Result<File> {
    std::fs::create_dir_all(dir)?;
    for n in (1..=KEEP).rev() {
        match std::fs::rename(numbered(dir, n - 1), numbered(dir, n)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    File::create(numbered(dir, 0))
}

/// Penulis ganda: stderr + berkas. Berkas dilepas (hanya stderr yang
/// tersisa) begitu gagal ditulis atau mencapai [`MAX_FILE_BYTES`].
struct Tee {
    file: Option<File>,
    written: u64,
}

impl Tee {
    fn write_file(&mut self, buf: &[u8]) {
        let Some(file) = self.file.as_mut() else {
            return;
        };
        let next = self.written.saturating_add(buf.len() as u64);
        if next > MAX_FILE_BYTES {
            let _ = file.write_all(b"[log dihentikan: batas ukuran tercapai]\n");
            self.file = None;
            return;
        }
        if file.write_all(buf).is_ok() {
            self.written = next;
        } else {
            self.file = None;
        }
    }
}

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        self.write_file(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        if let Some(file) = self.file.as_mut() {
            let _ = file.flush();
        }
        Ok(())
    }
}

/// Jurnal panik (diperkenalkan untuk soak 0.6.0-beta, PLAN_MIGRASI_TRUCK
/// F6.3): satu baris JSON per panik di [`log_dir`], TIDAK ikut dirotasi tiap
/// start sehingga bisa dihitung per versi. Hanya lokal — tidak pernah
/// dikirim ke mana pun; pengguna beta melampirkannya ke laporan bila diminta.
pub const PANIC_JOURNAL: &str = "panics.jsonl";
/// Jurnal berhenti tumbuh setelah ukuran ini.
pub const MAX_JOURNAL_BYTES: u64 = 1024 * 1024;

/// Satu entri jurnal: `{"ts","version","backend","thread","location","message"}`.
pub fn panic_record(
    ts: &str,
    thread: &str,
    location: Option<String>,
    message: &str,
) -> serde_json::Value {
    serde_json::json!({
        "ts": ts,
        "version": env!("CARGO_PKG_VERSION"),
        "backend": ducad_kernel::BACKEND,
        "thread": thread,
        "location": location,
        "message": message,
    })
}

/// Tambahkan `record` ke jurnal di `dir`. Gagal tulis atau jurnal penuh
/// diabaikan (tidak boleh memperparah panik).
pub fn append_panic_record(dir: &Path, record: &serde_json::Value) {
    let path = dir.join(PANIC_JOURNAL);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() >= MAX_JOURNAL_BYTES) {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{record}");
    }
}

/// Catat setiap panik ke log (lokasi + pesan) dan ke jurnal panik sebelum
/// hook bawaan mencetaknya ke stderr, supaya penyebab crash ada di berkas.
/// Panik yang kemudian ditangkap `catch_unwind` (op kernel, tool MCP) ikut
/// tercatat — justru itu yang dihitung selama soak backend kernel.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("PANIK: {info}");
        log::logger().flush();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".to_string());
        let record = panic_record(
            &chrono::Utc::now().to_rfc3339(),
            std::thread::current().name().unwrap_or("?"),
            info.location().map(|l| format!("{}:{}", l.file(), l.line())),
            &message,
        );
        append_panic_record(&log_dir(), &record);
        previous(info);
    }));
}

/// Pasang logger + panic hook. Mengembalikan path berkas log bila berhasil
/// dibuat. Aman dipanggil sekali di awal `main`.
pub fn init() -> Option<PathBuf> {
    let dir = log_dir();
    let file = match rotate(&dir) {
        Ok(f) => Some(f),
        Err(e) => {
            eprintln!(
                "ducad: berkas log tidak bisa dibuat di {}: {e}",
                dir.display()
            );
            None
        }
    };
    let path = file.is_some().then(|| dir.join(LOG_FILE));
    let tee = Tee { file, written: 0 };
    let env = env_logger::Env::default().default_filter_or(DEFAULT_FILTER);
    let _ = env_logger::Builder::from_env(env)
        .write_style(env_logger::WriteStyle::Never)
        .target(env_logger::Target::Pipe(Box::new(tee)))
        .try_init();
    install_panic_hook();
    log::info!(
        "DUCAD {} (kernel {}) dimulai; log: {}",
        env!("CARGO_PKG_VERSION"),
        ducad_kernel::BACKEND,
        path.as_ref()
            .map_or_else(|| "stderr saja".to_string(), |p| p.display().to_string())
    );
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ducad-log-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn rotate_keeps_previous_logs_and_drops_the_oldest() {
        let dir = temp_dir("rotate");
        for i in 0..4 {
            let mut f = rotate(&dir).unwrap();
            writeln!(f, "sesi {i}").unwrap();
        }
        let read = |n: usize| std::fs::read_to_string(numbered(&dir, n)).unwrap();
        assert_eq!(read(0).trim(), "sesi 3");
        assert_eq!(read(1).trim(), "sesi 2");
        assert_eq!(read(2).trim(), "sesi 1");
        assert!(!numbered(&dir, 3).exists(), "yang tertua dibuang");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn panic_journal_appends_one_json_line_per_panic_and_stops_at_cap() {
        let dir = temp_dir("journal");
        let rec = panic_record("2026-10-19T00:00:00+00:00", "main", Some("a.rs:1".into()), "boom");
        append_panic_record(&dir, &rec);
        append_panic_record(&dir, &rec);
        let text = std::fs::read_to_string(dir.join(PANIC_JOURNAL)).unwrap();
        let lines: Vec<serde_json::Value> =
            text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["backend"], ducad_kernel::BACKEND);
        assert_eq!(lines[0]["message"], "boom");
        assert_eq!(lines[0]["location"], "a.rs:1");

        let big = "x".repeat(MAX_JOURNAL_BYTES as usize);
        std::fs::write(dir.join(PANIC_JOURNAL), big).unwrap();
        append_panic_record(&dir, &rec);
        let len = std::fs::metadata(dir.join(PANIC_JOURNAL)).unwrap().len();
        assert_eq!(len, MAX_JOURNAL_BYTES, "jurnal penuh tidak ditambah");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tee_stops_writing_the_file_at_the_size_cap_but_never_fails() {
        let dir = temp_dir("cap");
        let file = rotate(&dir).unwrap();
        let mut tee = Tee {
            file: Some(file),
            written: MAX_FILE_BYTES - 4,
        };
        assert_eq!(tee.write(b"ab\n").unwrap(), 3);
        assert_eq!(tee.write(b"terlalu panjang\n").unwrap(), 16);
        assert!(tee.file.is_none(), "berkas dilepas setelah batas");
        assert_eq!(tee.write(b"masih ok\n").unwrap(), 9);
        let text = std::fs::read_to_string(numbered(&dir, 0)).unwrap();
        assert!(text.starts_with("ab\n[log dihentikan"), "{text:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
