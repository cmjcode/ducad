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

/// Catat setiap panik ke log (lokasi + pesan) sebelum hook bawaan
/// mencetaknya ke stderr, supaya penyebab crash ada di berkas log.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("PANIK: {info}");
        log::logger().flush();
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
        "DUCAD {} dimulai; log: {}",
        env!("CARGO_PKG_VERSION"),
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
