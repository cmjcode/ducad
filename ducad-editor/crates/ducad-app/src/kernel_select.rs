//! Pemilihan backend kernel saat runtime untuk masa soak 0.6.0-beta
//! (PLAN_MIGRASI_TRUCK F6.3).
//!
//! Backend kernel dipilih saat kompilasi (`ducad_kernel::BACKEND`), jadi satu
//! executable hanya membawa satu backend. Paket beta desktop membawa dua
//! executable bersebelahan:
//!
//! - `ducad`       — backend default (truck);
//! - `ducad-occt`  — backend OpenCASCADE, cadangan selama soak.
//!
//! `DUCAD_KERNEL=occt|truck` memilih backend. Bila backend yang diminta bukan
//! backend executable yang sedang berjalan, proses beralih ke executable
//! saudaranya (Unix: `exec`, PID & argumen sama; Windows: proses anak lalu
//! keluar dengan kode keluarnya). Executable saudara yang tidak ada, nilai yang
//! tidak dikenal, atau platform tablet (tanpa variabel lingkungan dan tanpa
//! `exec`) → tetap memakai backend bawaan dengan peringatan di log.
//!
//! `DUCAD_KERNEL_REEXEC` mencegah perpindahan berulang: proses hasil
//! perpindahan tidak pernah berpindah lagi walau backend-nya ternyata lain.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Variabel lingkungan pemilih backend.
pub const ENV: &str = "DUCAD_KERNEL";
/// Penanda bahwa proses ini sudah hasil perpindahan (cegah loop).
pub const REEXEC_ENV: &str = "DUCAD_KERNEL_REEXEC";
/// Backend yang dikenali.
pub const BACKENDS: [&str; 2] = ["truck", "occt"];

/// Keputusan pemilihan backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Pakai backend executable ini (tidak diminta, atau sudah cocok).
    Builtin,
    /// Pindah ke executable saudara ini.
    Switch(PathBuf),
    /// Tetap memakai backend bawaan; teks = alasan untuk log.
    Fallback(String),
}

/// Nama dasar executable tanpa akhiran backend dan tanpa `.exe`
/// (`ducad-occt.exe` → `ducad`).
fn base_stem(exe: &Path) -> String {
    let stem = exe
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ducad".to_string());
    for b in BACKENDS {
        if let Some(base) = stem.strip_suffix(&format!("-{b}")) {
            return base.to_string();
        }
    }
    stem
}

/// Calon executable untuk `backend`, urut prioritas: `<dasar>-<backend>`,
/// lalu `<dasar>` (executable utama = backend default). Executable yang
/// sedang berjalan tidak pernah jadi calon.
pub fn candidates(exe: &Path, backend: &str) -> Vec<PathBuf> {
    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let ext = exe
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let base = base_stem(exe);
    [format!("{base}-{backend}{ext}"), format!("{base}{ext}")]
        .into_iter()
        .map(|name| dir.join(name))
        .filter(|p| p.as_path() != exe)
        .collect()
}

/// Logika keputusan murni (diuji tanpa proses sungguhan).
pub fn decide(
    requested: Option<&str>,
    builtin: &str,
    exe: &Path,
    already_switched: bool,
    exists: impl Fn(&Path) -> bool,
) -> Choice {
    let Some(raw) = requested.map(str::trim).filter(|s| !s.is_empty()) else {
        return Choice::Builtin;
    };
    let wanted = raw.to_ascii_lowercase();
    if !BACKENDS.contains(&wanted.as_str()) {
        return Choice::Fallback(format!(
            "{ENV}={raw} tidak dikenal (pilihan: truck, occt); memakai backend {builtin}"
        ));
    }
    if wanted == builtin {
        return Choice::Builtin;
    }
    if already_switched {
        return Choice::Fallback(format!(
            "{ENV}={wanted} diminta, tetapi executable hasil perpindahan memakai backend \
             {builtin}; tidak berpindah lagi"
        ));
    }
    match candidates(exe, &wanted).into_iter().find(|p| exists(p)) {
        Some(path) => Choice::Switch(path),
        None => Choice::Fallback(format!(
            "{ENV}={wanted} diminta, tetapi executable backend {wanted} tidak ada di samping {} \
             (paket ini hanya membawa backend {builtin}); memakai backend {builtin}",
            exe.display()
        )),
    }
}

/// Dipanggil paling awal di `main`. Bila perlu, pindah ke executable backend
/// lain dan **tidak kembali**. Mengembalikan peringatan untuk dicatat setelah
/// logger aktif (perpindahan dilakukan sebelum log dirotasi, supaya proses
/// tujuan yang memiliki berkas log sesi ini).
pub fn switch_if_requested() -> Option<String> {
    if crate::platform::is_mobile() {
        return None;
    }
    let requested = std::env::var(ENV).ok();
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            return requested
                .map(|r| format!("{ENV}={r} diabaikan: lokasi executable tidak diketahui ({e})"));
        }
    };
    let already = std::env::var_os(REEXEC_ENV).is_some();
    match decide(requested.as_deref(), ducad_kernel::BACKEND, &exe, already, Path::exists) {
        Choice::Builtin => None,
        Choice::Fallback(msg) => Some(msg),
        Choice::Switch(target) => Some(run_sibling(&target)),
    }
}

/// Jalankan executable saudara dengan argumen yang sama. Hanya kembali bila
/// gagal (teks = alasan).
fn run_sibling(target: &Path) -> String {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut cmd = std::process::Command::new(target);
    cmd.args(&args).env(REEXEC_ENV, "1");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        format!(
            "gagal berpindah ke {}: {err}; memakai backend {}",
            target.display(),
            ducad_kernel::BACKEND
        )
    }
    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(err) => format!(
                "gagal menjalankan {}: {err}; memakai backend {}",
                target.display(),
                ducad_kernel::BACKEND
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe() -> PathBuf {
        PathBuf::from("/Applications/DUCAD.app/Contents/MacOS/ducad")
    }

    #[test]
    fn tanpa_permintaan_atau_sama_dengan_bawaan_tidak_berpindah() {
        assert_eq!(decide(None, "truck", &exe(), false, |_| true), Choice::Builtin);
        assert_eq!(decide(Some("  "), "truck", &exe(), false, |_| true), Choice::Builtin);
        assert_eq!(decide(Some("TRUCK"), "truck", &exe(), false, |_| true), Choice::Builtin);
    }

    #[test]
    fn berpindah_ke_executable_occt_di_sampingnya() {
        let want = PathBuf::from("/Applications/DUCAD.app/Contents/MacOS/ducad-occt");
        let got = decide(Some("occt"), "truck", &exe(), false, |p| p == want);
        assert_eq!(got, Choice::Switch(want));
    }

    #[test]
    fn dari_executable_occt_kembali_ke_executable_utama() {
        let occt = PathBuf::from("/opt/ducad/ducad-occt.exe");
        let main = PathBuf::from("/opt/ducad/ducad.exe");
        let got = decide(Some("truck"), "occt", &occt, false, |p| p == main);
        assert_eq!(got, Choice::Switch(main));
    }

    #[test]
    fn executable_tidak_ada_atau_nilai_asing_tetap_bawaan_dengan_alasan() {
        match decide(Some("occt"), "truck", &exe(), false, |_| false) {
            Choice::Fallback(msg) => assert!(msg.contains("tidak ada"), "{msg}"),
            other => panic!("{other:?}"),
        }
        match decide(Some("parasolid"), "truck", &exe(), false, |_| true) {
            Choice::Fallback(msg) => assert!(msg.contains("tidak dikenal"), "{msg}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn hasil_perpindahan_tidak_pernah_berpindah_lagi() {
        let got = decide(Some("occt"), "truck", &exe(), true, |_| true);
        assert!(matches!(got, Choice::Fallback(_)), "{got:?}");
    }

    #[test]
    fn executable_sendiri_bukan_calon() {
        let me = PathBuf::from("/x/ducad-occt");
        assert_eq!(candidates(&me, "occt"), vec![PathBuf::from("/x/ducad")]);
    }
}
