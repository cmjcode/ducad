//! Mengunduh, memverifikasi, dan memasang paket rilis di atas instalasi
//! yang sedang berjalan.
//!
//! - **Windows** (`.zip`) dan **Linux** (`.tar.gz`): isi paket disalin ke
//!   folder executable. Tiap berkas ditulis dulu sebagai `.<nama>.ducad-new`
//!   lalu ditukar lewat rename; berkas lama diganti nama `.<nama>.ducad-old`.
//!   Windows tidak mengizinkan executable yang sedang berjalan ditimpa atau
//!   dihapus, tetapi MENGIZINKAN ganti nama — karena itu berkas lama
//!   disingkirkan, bukan ditimpa, dan baru dihapus [`cleanup_leftovers`] saat
//!   start berikutnya.
//! - **macOS** (`.dmg`): dmg di-mount read-only, bundel `.app` disalin
//!   dengan `ditto` ke sebelah bundel lama, tanda tangannya diverifikasi
//!   (`codesign`, Team ID harus sama dengan bundel yang berjalan), lalu kedua
//!   bundel ditukar dengan rename.
//!
//! Integritas: paket Windows/Linux WAJIB punya `<paket>.sha256` (selalu
//! dibuat `release.yml`). Di macOS, bila bundel yang berjalan ditandatangani
//! Developer ID, bundel baru wajib lolos `codesign` dengan Team ID yang sama
//! (checksum opsional). Bundel ad-hoc/tanpa tanda tangan (build lokal
//! `make bundle-macos` tanpa `APPLE_IDENTITY`) tidak punya identitas untuk
//! dicocokkan, jadi untuknya `<dmg>.sha256` WAJIB.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::release::{user_agent, Asset, Os, Release, Target};
use crate::version::Version;
use crate::UpdateError;

/// Batas ukuran paket; paket DUCAD jauh di bawah ini.
const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_CHECKSUM_BYTES: u64 = 4096;

const NEW_SUFFIX: &str = ".ducad-new";
const OLD_SUFFIX: &str = ".ducad-old";

/// Kemajuan unduhan, dibaca thread GUI selagi thread latar mengunduh.
#[derive(Debug, Default)]
pub struct Progress {
    downloaded: AtomicU64,
    total: AtomicU64,
}

impl Progress {
    pub fn downloaded(&self) -> u64 {
        self.downloaded.load(Ordering::Relaxed)
    }

    /// 0 = ukuran belum diketahui.
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    pub fn fraction(&self) -> Option<f32> {
        let total = self.total();
        (total > 0).then(|| (self.downloaded() as f64 / total as f64).clamp(0.0, 1.0) as f32)
    }

    fn start(&self, total: u64) {
        self.downloaded.store(0, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
    }

    fn add(&self, bytes: u64) {
        self.downloaded.fetch_add(bytes, Ordering::Relaxed);
    }
}

/// Alasan instalasi yang berjalan tidak bisa memperbarui dirinya sendiri;
/// pengguna diarahkan ke halaman unduhan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsupported {
    /// OS tanpa paket desktop (iOS, Android, …).
    Platform,
    /// Build pengembangan (`target/debug|release`) atau macOS di luar `.app`.
    NotPackaged,
    /// Dipasang dari Mac App Store — pembaruan lewat App Store.
    AppStore,
    /// Folder instalasi tidak bisa ditulis (mis. `/usr/bin` dari paket AUR,
    /// `Program Files` tanpa hak admin).
    ReadOnly(PathBuf),
}

/// Instalasi yang sedang berjalan.
#[derive(Debug, Clone)]
pub struct Installation {
    /// Windows/Linux: folder berisi executable. macOS: bundel `.app`.
    pub root: PathBuf,
    exe_name: OsString,
    os: Os,
}

impl Installation {
    /// Instalasi proses ini, sekaligus memeriksa bahwa ia bisa diperbarui.
    pub fn current() -> Result<Self, Unsupported> {
        Self::locate(true)
    }

    fn locate(require_writable: bool) -> Result<Self, Unsupported> {
        let target = Target::current().ok_or(Unsupported::Platform)?;
        let exe = std::env::current_exe()
            .and_then(fs::canonicalize)
            .map_err(|_| Unsupported::NotPackaged)?;
        let inst = Self::from_exe(&exe, target.os)?;
        if require_writable {
            let dir = inst.writable_dir();
            if !dir_writable(&dir) {
                return Err(Unsupported::ReadOnly(dir));
            }
        }
        Ok(inst)
    }

    fn from_exe(exe: &Path, os: Os) -> Result<Self, Unsupported> {
        if is_dev_build(exe) {
            return Err(Unsupported::NotPackaged);
        }
        let exe_name = exe.file_name().ok_or(Unsupported::NotPackaged)?.to_os_string();
        let root = match os {
            Os::MacOs => {
                let bundle = app_bundle_of(exe).ok_or(Unsupported::NotPackaged)?;
                if bundle.join("Contents").join("_MASReceipt").exists() {
                    return Err(Unsupported::AppStore);
                }
                bundle
            }
            Os::Windows | Os::Linux => exe.parent().ok_or(Unsupported::NotPackaged)?.to_path_buf(),
        };
        Ok(Self { root, exe_name, os })
    }

    /// Folder yang isinya diganti: folder executable, atau induk bundel `.app`.
    fn writable_dir(&self) -> PathBuf {
        match self.os {
            Os::MacOs => self.root.parent().map(Path::to_path_buf).unwrap_or_else(|| self.root.clone()),
            Os::Windows | Os::Linux => self.root.clone(),
        }
    }
}

/// Hasil pemasangan yang berhasil; versi baru aktif setelah mulai ulang.
#[derive(Debug, Clone)]
pub struct Installed {
    pub version: Version,
    root: PathBuf,
    exe_name: OsString,
    os: Os,
}

impl Installed {
    /// Menjalankan versi baru sebagai proses terpisah. Panggil tepat sebelum
    /// proses ini keluar.
    pub fn relaunch(&self) -> std::io::Result<()> {
        match self.os {
            // `open -n`: instans baru walau proses lama belum benar-benar keluar.
            Os::MacOs => Command::new("/usr/bin/open").arg("-n").arg(&self.root).spawn().map(drop),
            Os::Windows | Os::Linux => Command::new(self.root.join(&self.exe_name)).spawn().map(drop),
        }
    }
}

/// Mengunduh paket `release` untuk sistem ini, memverifikasinya, lalu
/// memasangnya di atas instalasi yang berjalan. Memblokir (bisa beberapa
/// menit) — panggil dari thread latar.
pub fn install(release: &Release, progress: &Progress) -> Result<Installed, UpdateError> {
    let inst = Installation::current().map_err(UpdateError::Unsupported)?;
    let target = Target::current().ok_or(UpdateError::Unsupported(Unsupported::Platform))?;
    let package = release.package_for(&target).ok_or(UpdateError::NoPackage)?;

    let work = WorkDir::create(&release.version)?;
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .build();
    let file_name = Path::new(&package.name)
        .file_name()
        .ok_or_else(|| UpdateError::Io(format!("nama paket tidak valid: {}", package.name)))?;
    let archive = work.path.join(file_name);
    let digest = download(&agent, package, &archive, progress)?;

    let checksum_verified = match release.checksum_for(package) {
        Some(sum) => {
            let expected = fetch_checksum(&agent, sum)?;
            if !expected.eq_ignore_ascii_case(&digest) {
                return Err(UpdateError::ChecksumMismatch);
            }
            true
        }
        // macOS boleh tanpa checksum asal tanda tangan kode bisa dicocokkan;
        // diputuskan `verify_codesign`.
        None if target.os == Os::MacOs => false,
        None => return Err(UpdateError::ChecksumMissing),
    };

    apply(&inst, &archive, &work.path, checksum_verified)?;
    log::info!("Pembaruan {} terpasang di {}", release.version, inst.root.display());
    Ok(Installed {
        version: release.version.clone(),
        root: inst.root,
        exe_name: inst.exe_name,
        os: inst.os,
    })
}

/// Menghapus sisa pemasangan sebelumnya (`*.ducad-old`, `*.ducad-new`).
/// Dipanggil saat start, ketika berkas lama sudah tidak dipakai proses mana
/// pun.
pub fn cleanup_leftovers() {
    let Ok(inst) = Installation::locate(false) else {
        return;
    };
    let Ok(entries) = fs::read_dir(inst.writable_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with('.') && (name.ends_with(OLD_SUFFIX) || name.ends_with(NEW_SUFFIX))) {
            continue;
        }
        let path = entry.path();
        let removed = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
        if let Err(e) = removed {
            log::debug!("Sisa pembaruan {} belum bisa dihapus: {e}", path.display());
        }
    }
}

// ---------------------------------------------------------------------------
// Unduhan & checksum

fn download(
    agent: &ureq::Agent,
    asset: &Asset,
    dest: &Path,
    progress: &Progress,
) -> Result<String, UpdateError> {
    if !asset.browser_download_url.starts_with("https://") {
        return Err(UpdateError::Network("URL paket harus https".to_string()));
    }
    let response = agent
        .get(&asset.browser_download_url)
        .set("User-Agent", &user_agent())
        .set("Accept", "application/octet-stream")
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    let total = response
        .header("Content-Length")
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(asset.size);
    if total > MAX_PACKAGE_BYTES {
        return Err(UpdateError::Network("ukuran paket tidak wajar".to_string()));
    }
    progress.start(total);

    let mut reader = response.into_reader().take(MAX_PACKAGE_BYTES + 1);
    let mut file = File::create(dest)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let n = reader.read(&mut buf).map_err(|e| UpdateError::Network(e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        written += n as u64;
        progress.add(n as u64);
        if written > MAX_PACKAGE_BYTES {
            return Err(UpdateError::Network("ukuran paket tidak wajar".to_string()));
        }
    }
    file.sync_all()?;
    if asset.size > 0 && written != asset.size {
        return Err(UpdateError::Network(format!(
            "unduhan tidak lengkap ({written} dari {} byte)",
            asset.size
        )));
    }
    Ok(hex::encode(hasher.finalize()))
}

fn fetch_checksum(agent: &ureq::Agent, asset: &Asset) -> Result<String, UpdateError> {
    let response = agent
        .get(&asset.browser_download_url)
        .set("User-Agent", &user_agent())
        .set("Accept", "application/octet-stream")
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    let mut text = String::new();
    response
        .into_reader()
        .take(MAX_CHECKSUM_BYTES)
        .read_to_string(&mut text)
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    parse_checksum(&text).ok_or(UpdateError::ChecksumMismatch)
}

/// Format `sha256sum`: `<64 hex>  <nama berkas>`.
fn parse_checksum(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    (token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| token.to_ascii_lowercase())
}

// ---------------------------------------------------------------------------
// Pemasangan per platform

#[cfg(target_os = "linux")]
fn apply(inst: &Installation, archive: &Path, work: &Path, _checksum_verified: bool) -> Result<(), UpdateError> {
    let out = work.join("pkg");
    fs::create_dir_all(&out)?;
    run(Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(&out))?;
    replace_files(&package_root(&out)?, &inst.root, &inst.exe_name, false)
}

#[cfg(target_os = "windows")]
fn apply(inst: &Installation, archive: &Path, work: &Path, _checksum_verified: bool) -> Result<(), UpdateError> {
    let out = work.join("pkg");
    fs::create_dir_all(&out)?;
    let mut zip = zip::ZipArchive::new(File::open(archive)?)
        .map_err(|e| UpdateError::Io(format!("zip rusak: {e}")))?;
    zip.extract(&out).map_err(|e| UpdateError::Io(format!("gagal mengekstrak zip: {e}")))?;
    replace_files(&package_root(&out)?, &inst.root, &inst.exe_name, true)
}

#[cfg(target_os = "macos")]
fn apply(inst: &Installation, dmg: &Path, work: &Path, checksum_verified: bool) -> Result<(), UpdateError> {
    let mount = work.join("mnt");
    fs::create_dir_all(&mount)?;
    run(Command::new("/usr/bin/hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-quiet", "-mountpoint"])
        .arg(&mount)
        .arg(dmg))?;
    // Dilepas sebelum `WorkDir` (milik pemanggil) menghapus folder kerja.
    let _detach = Detach(mount.clone());

    let new_app = fs::read_dir(&mount)?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension() == Some(OsStr::new("app")) && p.is_dir())
        .ok_or_else(|| UpdateError::Io("dmg tidak berisi bundel .app".to_string()))?;

    let parent = inst.writable_dir();
    let bundle_name = inst.root.file_name().unwrap_or_else(|| OsStr::new("DUCAD.app"));
    let staged = parent.join(hidden_name(bundle_name, NEW_SUFFIX));
    let backup = parent.join(hidden_name(bundle_name, OLD_SUFFIX));
    remove_path(&staged);
    run(Command::new("/usr/bin/ditto").arg(&new_app).arg(&staged))?;
    if let Err(e) = verify_codesign(&staged, &inst.root, checksum_verified) {
        remove_path(&staged);
        return Err(e);
    }

    // Bundel lama disingkirkan, bukan dihapus: proses yang sedang berjalan
    // masih bisa memuat sumber daya dari dalamnya. `cleanup_leftovers`
    // membuangnya saat start berikutnya.
    remove_path(&backup);
    fs::rename(&inst.root, &backup)?;
    if let Err(e) = fs::rename(&staged, &inst.root) {
        let _ = fs::rename(&backup, &inst.root);
        remove_path(&staged);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn apply(
    _inst: &Installation,
    _archive: &Path,
    _work: &Path,
    _checksum_verified: bool,
) -> Result<(), UpdateError> {
    Err(UpdateError::Unsupported(Unsupported::Platform))
}

#[cfg(target_os = "macos")]
struct Detach(PathBuf);

#[cfg(target_os = "macos")]
impl Drop for Detach {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/hdiutil")
            .args(["detach", "-quiet", "-force"])
            .arg(&self.0)
            .status();
    }
}

/// Bundel yang berjalan bertanda tangan Developer ID: bundel baru wajib
/// lolos `codesign --verify --deep --strict` dengan Team ID yang sama.
/// Bundel ad-hoc/tanpa tanda tangan tidak punya identitas untuk dicocokkan
/// (dan segel bundel hasil `cargo bundle` tanpa `APPLE_IDENTITY` memang
/// tidak valid), jadi integritasnya harus datang dari checksum SHA-256.
#[cfg(target_os = "macos")]
fn verify_codesign(new_app: &Path, current_app: &Path, checksum_verified: bool) -> Result<(), UpdateError> {
    let Some(expected) = team_id(current_app) else {
        return if checksum_verified { Ok(()) } else { Err(UpdateError::ChecksumMissing) };
    };
    run(Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict"]).arg(new_app))
        .map_err(|e| UpdateError::Signature(e.to_string()))?;
    match team_id(new_app) {
        Some(found) if found == expected => Ok(()),
        found => Err(UpdateError::Signature(format!(
            "Team ID {} tidak sama dengan {expected}",
            found.as_deref().unwrap_or("kosong")
        ))),
    }
}

#[cfg(target_os = "macos")]
fn team_id(app: &Path) -> Option<String> {
    let out = Command::new("/usr/bin/codesign").args(["-dv", "--verbose=2"]).arg(app).output().ok()?;
    // `codesign -d` menulis informasinya ke stderr.
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .find_map(|l| l.strip_prefix("TeamIdentifier="))
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "not set")
        .map(str::to_string)
}

/// Paket CI berisi satu folder (`ducad-<versi>-<platform>/`); isi folder
/// itulah yang dipasang. Arsip tanpa folder pembungkus dipakai apa adanya.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn package_root(extracted: &Path) -> Result<PathBuf, UpdateError> {
    let entries: Vec<PathBuf> = fs::read_dir(extracted)?.flatten().map(|e| e.path()).collect();
    match entries.as_slice() {
        [only] if only.is_dir() => Ok(only.clone()),
        _ => Ok(extracted.to_path_buf()),
    }
}

/// Menyalin semua berkas di `src_dir` ke `dest_dir`. Tiap berkas ditulis
/// sebagai `.<nama>.ducad-new`, berkas lama diganti nama `.<nama>.ducad-old`,
/// lalu yang baru di-rename ke tempatnya. Bila satu langkah gagal, berkas
/// yang sudah ditukar dikembalikan. `keep_backups`: biarkan `.ducad-old`
/// (Windows: executable yang berjalan tidak bisa dihapus).
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn replace_files(
    src_dir: &Path,
    dest_dir: &Path,
    required: &OsStr,
    keep_backups: bool,
) -> Result<(), UpdateError> {
    let files: Vec<PathBuf> =
        fs::read_dir(src_dir)?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    if !files.iter().any(|p| p.file_name() == Some(required)) {
        return Err(UpdateError::Io(format!(
            "paket tidak berisi {}",
            required.to_string_lossy()
        )));
    }

    // (tujuan, cadangan berkas lama bila ada)
    let mut swapped: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let rollback = |swapped: &[(PathBuf, Option<PathBuf>)]| {
        for (target, backup) in swapped.iter().rev() {
            let _ = fs::remove_file(target);
            if let Some(backup) = backup {
                let _ = fs::rename(backup, target);
            }
        }
    };

    for src in &files {
        let Some(name) = src.file_name() else { continue };
        let target = dest_dir.join(name);
        let staged = dest_dir.join(hidden_name(name, NEW_SUFFIX));
        let backup = dest_dir.join(hidden_name(name, OLD_SUFFIX));
        let step = || -> std::io::Result<Option<PathBuf>> {
            // `fs::copy` ikut menyalin bit izin (executable di Unix).
            fs::copy(src, &staged)?;
            let moved_old = if target.exists() {
                let _ = fs::remove_file(&backup);
                fs::rename(&target, &backup)?;
                Some(backup.clone())
            } else {
                None
            };
            if let Err(e) = fs::rename(&staged, &target) {
                if let Some(b) = &moved_old {
                    let _ = fs::rename(b, &target);
                }
                return Err(e);
            }
            Ok(moved_old)
        };
        match step() {
            Ok(moved_old) => swapped.push((target, moved_old)),
            Err(e) => {
                let _ = fs::remove_file(&staged);
                rollback(&swapped);
                return Err(UpdateError::Io(format!(
                    "gagal mengganti {}: {e}",
                    target.display()
                )));
            }
        }
    }

    if !keep_backups {
        for backup in swapped.iter().filter_map(|(_, b)| b.as_ref()) {
            let _ = fs::remove_file(backup);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Utilitas

/// Folder kerja sementara; dihapus saat di-drop (berhasil maupun gagal).
struct WorkDir {
    path: PathBuf,
}

impl WorkDir {
    fn create(version: &Version) -> Result<Self, UpdateError> {
        let path =
            std::env::temp_dir().join(format!("ducad-update-{}-{}", version, std::process::id()));
        remove_path(&path);
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn hidden_name(name: &OsStr, suffix: &str) -> OsString {
    let mut out = OsString::from(".");
    out.push(name);
    out.push(suffix);
    out
}

fn remove_path(path: &Path) {
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else if path.exists() {
        let _ = fs::remove_file(path);
    }
}

#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
fn run(cmd: &mut Command) -> Result<(), UpdateError> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let out = cmd.output().map_err(|e| UpdateError::Io(format!("{program}: {e}")))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    Err(UpdateError::Io(format!("{program} gagal ({}): {}", out.status, stderr.trim())))
}

fn dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".ducad-write-test-{}", std::process::id()));
    match File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Executable hasil `cargo build` (`…/target/[<triple>/]debug|release/…`).
fn is_dev_build(exe: &Path) -> bool {
    let parts: Vec<&OsStr> = exe.components().map(|c| c.as_os_str()).collect();
    parts.iter().enumerate().any(|(i, part)| {
        *part == "target"
            && parts[i + 1..].iter().take(2).any(|p| *p == "debug" || *p == "release")
    })
}

/// Bundel `.app` yang memuat `exe` (`X.app/Contents/MacOS/x`).
fn app_bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos_dir = exe.parent()?;
    let contents = macos_dir.parent()?;
    let bundle = contents.parent()?;
    (macos_dir.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then(|| bundle.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ducad-update-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn checksum_parsing() {
        let hash = "A".repeat(64);
        assert_eq!(parse_checksum(&format!("{hash}  ducad.zip\n")), Some("a".repeat(64)));
        assert_eq!(parse_checksum("abc  ducad.zip"), None);
        assert_eq!(parse_checksum(""), None);
        assert_eq!(parse_checksum(&"g".repeat(64)), None);
    }

    #[test]
    fn detects_dev_builds_and_bundles() {
        assert!(is_dev_build(Path::new("/src/ducad-editor/target/release/ducad")));
        assert!(is_dev_build(Path::new("/src/target/aarch64-apple-darwin/debug/ducad")));
        assert!(!is_dev_build(Path::new("/opt/ducad-0.4.6-linux-x86_64/ducad")));
        assert_eq!(
            app_bundle_of(Path::new("/Applications/DUCAD.app/Contents/MacOS/ducad")),
            Some(PathBuf::from("/Applications/DUCAD.app"))
        );
        assert_eq!(app_bundle_of(Path::new("/usr/local/bin/ducad")), None);
    }

    #[test]
    fn from_exe_resolves_root_per_os() {
        let linux = Installation::from_exe(Path::new("/opt/ducad/ducad"), Os::Linux).unwrap();
        assert_eq!(linux.root, PathBuf::from("/opt/ducad"));
        assert_eq!(
            Installation::from_exe(Path::new("/opt/ducad/ducad"), Os::MacOs).unwrap_err(),
            Unsupported::NotPackaged
        );
        let bundle = temp("mas").join("DUCAD.app");
        fs::create_dir_all(bundle.join("Contents/_MASReceipt")).unwrap();
        assert_eq!(
            Installation::from_exe(&bundle.join("Contents/MacOS/ducad"), Os::MacOs).unwrap_err(),
            Unsupported::AppStore
        );
    }

    #[test]
    fn package_root_unwraps_single_folder() {
        let dir = temp("root");
        fs::create_dir_all(dir.join("ducad-0.4.6-linux-x86_64")).unwrap();
        assert_eq!(package_root(&dir).unwrap(), dir.join("ducad-0.4.6-linux-x86_64"));
        fs::write(dir.join("README"), b"x").unwrap();
        assert_eq!(package_root(&dir).unwrap(), dir);
    }

    #[test]
    fn replace_files_swaps_and_keeps_or_drops_backups() {
        let base = temp("replace");
        let (src, dest) = (base.join("src"), base.join("dest"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dest).unwrap();
        fs::write(src.join("ducad"), b"new").unwrap();
        fs::write(src.join("ducad-cli"), b"new-cli").unwrap();
        fs::write(dest.join("ducad"), b"old").unwrap();
        fs::write(dest.join("user-notes.txt"), b"keep").unwrap();

        replace_files(&src, &dest, OsStr::new("ducad"), false).unwrap();
        assert_eq!(fs::read(dest.join("ducad")).unwrap(), b"new");
        assert_eq!(fs::read(dest.join("ducad-cli")).unwrap(), b"new-cli");
        assert_eq!(fs::read(dest.join("user-notes.txt")).unwrap(), b"keep");
        assert!(!dest.join(".ducad.ducad-old").exists());
        assert!(!dest.join(".ducad.ducad-new").exists());

        fs::write(src.join("ducad"), b"newer").unwrap();
        replace_files(&src, &dest, OsStr::new("ducad"), true).unwrap();
        assert_eq!(fs::read(dest.join("ducad")).unwrap(), b"newer");
        assert_eq!(fs::read(dest.join(".ducad.ducad-old")).unwrap(), b"new");
    }

    #[test]
    fn replace_files_refuses_package_without_main_exe() {
        let base = temp("refuse");
        let (src, dest) = (base.join("src"), base.join("dest"));
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dest).unwrap();
        fs::write(src.join("other"), b"x").unwrap();
        fs::write(dest.join("ducad"), b"old").unwrap();
        assert!(replace_files(&src, &dest, OsStr::new("ducad"), false).is_err());
        assert_eq!(fs::read(dest.join("ducad")).unwrap(), b"old");
        assert!(!dest.join("other").exists());
    }
}
