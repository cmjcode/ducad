//! Preferensi updater di `<data_dir>/update.json` (desktop: `~/.ducad`).
//!
//! ```json
//! {"auto_check": true, "last_check_unix": 1760432000,
//!  "skipped_version": "0.4.6", "pending_notice": null}
//! ```

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "update.json";

/// Jarak minimum antar pemeriksaan otomatis.
pub const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    /// Periksa rilis baru saat aplikasi dibuka (maks. sekali per
    /// [`CHECK_INTERVAL_SECS`]).
    pub auto_check: bool,
    /// Waktu pemeriksaan terakhir (detik Unix).
    pub last_check_unix: Option<u64>,
    /// Versi yang dipilih "Lewati"; pemeriksaan otomatis tidak menawarkannya
    /// lagi (pemeriksaan manual tetap menampilkannya).
    pub skipped_version: Option<String>,
    /// Versi yang baru dipasang sebelum mulai ulang; ditampilkan sekali saat
    /// start berikutnya lalu dihapus.
    pub pending_notice: Option<String>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self { auto_check: true, last_check_unix: None, skipped_version: None, pending_notice: None }
    }
}

impl UpdateSettings {
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(FILE_NAME)
    }

    /// Berkas hilang/rusak = nilai bawaan (bukan galat).
    pub fn load(dir: &Path) -> Self {
        std::fs::read(Self::path(dir))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(Self::path(dir), json)
    }

    /// Pemeriksaan otomatis jatuh tempo pada `now` (detik Unix)? Jam yang
    /// mundur dianggap jatuh tempo, supaya tidak macet selamanya.
    pub fn check_due(&self, now: u64) -> bool {
        self.auto_check
            && self
                .last_check_unix
                .is_none_or(|last| now < last || now - last >= CHECK_INTERVAL_SECS)
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_due_respects_interval_and_toggle() {
        let mut s = UpdateSettings::default();
        assert!(s.check_due(1_000));
        s.last_check_unix = Some(1_000);
        assert!(!s.check_due(1_000 + CHECK_INTERVAL_SECS - 1));
        assert!(s.check_due(1_000 + CHECK_INTERVAL_SECS));
        assert!(s.check_due(500), "jam mundur tetap memicu pemeriksaan");
        s.auto_check = false;
        assert!(!s.check_due(u64::MAX));
    }

    #[test]
    fn load_save_round_trip_and_tolerates_garbage() {
        let dir = std::env::temp_dir().join(format!("ducad-update-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(UpdateSettings::load(&dir), UpdateSettings::default());

        let s = UpdateSettings {
            auto_check: false,
            last_check_unix: Some(7),
            skipped_version: Some("0.4.6".into()),
            pending_notice: None,
        };
        s.save(&dir).unwrap();
        assert_eq!(UpdateSettings::load(&dir), s);

        std::fs::write(UpdateSettings::path(&dir), b"{not json").unwrap();
        assert_eq!(UpdateSettings::load(&dir), UpdateSettings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
