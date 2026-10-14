//! `ducad-update`: pembaruan otomatis aplikasi desktop DUCAD (Windows,
//! Linux, macOS) dari GitHub Releases.
//!
//! Alur yang dipakai `ducad-app`:
//! 1. [`cleanup_leftovers`] saat start (sisa pembaruan sebelumnya).
//! 2. [`check_latest`] di thread latar — `Some(release)` bila ada versi baru.
//! 3. [`install`] di thread latar — unduh, verifikasi SHA-256/codesign,
//!    tukar berkas instalasi. [`Installation::current`] memberi tahu lebih
//!    dulu apakah instalasi ini bisa memperbarui dirinya sendiri.
//! 4. [`Installed::relaunch`] tepat sebelum proses keluar.

// Prinsip "jangan pernah crash" (ADR 0009): tidak ada unwrap/expect/panic di
// kode produksi; modul tes dikecualikan.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod install;
mod release;
mod settings;
mod version;

pub use install::{cleanup_leftovers, install, Installation, Installed, Progress, Unsupported};
pub use release::{fetch_latest, repo, select_package, Asset, Os, Release, Target, DEFAULT_REPO, REPO_ENV};
pub use settings::{now_unix, UpdateSettings, CHECK_INTERVAL_SECS};
pub use version::Version;

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("jaringan: {0}")]
    Network(String),
    #[error("belum ada rilis")]
    NoRelease,
    #[error("rilis tidak menyediakan paket untuk sistem ini")]
    NoPackage,
    #[error("berkas checksum SHA-256 tidak tersedia")]
    ChecksumMissing,
    #[error("checksum SHA-256 tidak cocok")]
    ChecksumMismatch,
    #[error("tanda tangan kode: {0}")]
    Signature(String),
    #[error("instalasi tidak bisa diperbarui sendiri: {0:?}")]
    Unsupported(Unsupported),
    #[error("{0}")]
    Io(String),
}

impl From<std::io::Error> for UpdateError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Memeriksa rilis terbaru di [`repo`]. `Ok(Some(_))` bila lebih baru dari
/// `current` (biasanya `env!("CARGO_PKG_VERSION")`). Memblokir — panggil
/// dari thread latar.
pub fn check_latest(current: &str) -> Result<Option<Release>, UpdateError> {
    let current = Version::parse(current)
        .ok_or_else(|| UpdateError::Io(format!("versi aplikasi tidak valid: {current}")))?;
    let release = fetch_latest(&repo())?;
    Ok((release.version > current).then_some(release))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Menyentuh GitHub sungguhan: `cargo test -p ducad-update -- --ignored`.
    #[test]
    #[ignore]
    fn live_latest_release_has_package_for_this_system() {
        let release = check_latest("0.0.1").unwrap().expect("ada rilis lebih baru dari 0.0.1");
        let target = Target::current().unwrap();
        let package = release.package_for(&target);
        println!("{} -> {:?}", release.tag, package.map(|a| &a.name));
        assert!(package.is_some(), "rilis {} tanpa paket untuk {target:?}", release.tag);
    }
}
