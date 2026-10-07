//! Abstraksi platform: desktop vs tablet (iPadOS, Android).
//!
//! Semua keputusan "ini tablet" dan semua jalur berkas per-platform
//! dikumpulkan di sini supaya modul lain tidak menebar `cfg(target_os)`
//! sendiri-sendiri. Prinsip:
//!
//! - **Mobile** = iPadOS atau Android: sentuh sebagai input utama, tidak ada
//!   proses anak/soket Unix untuk agent, memori terbatas (aplikasi bisa
//!   dimatikan OS kapan saja), dan GPU hemat daya.
//! - Jalur data di tablet selalu di dalam sandbox aplikasi. Di Android,
//!   `android_main` (crate `ducad-android`) mengeset `HOME` ke folder data
//!   internal sebelum apa pun berjalan, sehingga kode `$HOME/.ducad` yang
//!   sudah ada (riwayat, konfigurasi chat, kunci API) ikut bekerja tanpa
//!   perubahan.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Nama env var yang diisi `ducad-android` dengan folder dokumen yang
/// terlihat pengguna (`getExternalFilesDir`).
pub const DOCUMENTS_DIR_ENV: &str = "DUCAD_DOCUMENTS_DIR";

/// Benar bila dikompilasi untuk tablet (iPadOS/Android).
pub const IS_MOBILE: bool = cfg!(any(target_os = "ios", target_os = "android"));

/// Lihat [`IS_MOBILE`]; bentuk fungsi untuk dipakai dalam ekspresi.
pub fn is_mobile() -> bool {
    IS_MOBILE
}

/// Sentuh sebagai input utama: selalu di tablet, atau di desktop bila
/// jendela sempit (uji tata letak tablet lewat mouse/trackpad).
pub fn touch_first(screen_width: f32) -> bool {
    IS_MOBILE || screen_width < 1050.0
}

/// Nilai bawaan backdrop GPU Liquid Glass. Di tablet dimatikan: blur
/// offscreen tiap frame mahal di GPU hemat daya dan terasa panas/boros
/// baterai; panel memakai isian datar (tetap bisa dinyalakan di ⚙).
pub fn glass_gpu_default() -> bool {
    !IS_MOBILE
}

/// Folder data aplikasi (riwayat, preferensi, autosave).
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "ios")]
    {
        crate::apple::apple_app_support_directory().join("DUCAD")
    }
    #[cfg(not(target_os = "ios"))]
    {
        match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".ducad"),
            None => PathBuf::from(".ducad"),
        }
    }
}

/// Folder dokumen yang terlihat pengguna (Files.app "Di iPad Ini ▸ DUCAD",
/// Android `Android/data/<paket>/files`, desktop `$HOME/Documents`).
pub fn documents_dir() -> PathBuf {
    #[cfg(target_os = "ios")]
    {
        crate::apple::apple_documents_directory()
    }
    #[cfg(not(target_os = "ios"))]
    {
        if let Some(dir) = std::env::var_os(DOCUMENTS_DIR_ENV) {
            return PathBuf::from(dir);
        }
        match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join("Documents"),
            None => PathBuf::from("Documents"),
        }
    }
}

/// Berkas pemulihan: ditulis saat aplikasi ditidurkan OS (tablet) dan
/// berkala, dibaca kembali saat start bila ada.
pub fn autosave_path() -> PathBuf {
    data_dir().join("autosave.ducad")
}

/// Jalur berkas penanda bahwa `autosave.ducad` milik dokumen `path`
/// (supaya pemulihan membuka ulang nama berkas yang benar).
pub fn autosave_origin_path() -> PathBuf {
    data_dir().join("autosave.origin")
}

static MEMORY_WARNING: AtomicBool = AtomicBool::new(false);

/// Dipanggil observer OS (iOS `UIApplicationDidReceiveMemoryWarning`) dari
/// thread mana pun; aplikasi membacanya di awal frame berikutnya.
pub fn signal_memory_warning() {
    MEMORY_WARNING.store(true, Ordering::SeqCst);
}

/// Ambil dan reset sinyal peringatan memori.
pub fn take_memory_warning() -> bool {
    MEMORY_WARNING.swap(false, Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_first_follows_width_on_desktop() {
        if !IS_MOBILE {
            assert!(touch_first(800.0));
            assert!(!touch_first(1600.0));
        }
    }

    #[test]
    fn memory_warning_is_edge_triggered() {
        assert!(!take_memory_warning());
        signal_memory_warning();
        assert!(take_memory_warning());
        assert!(!take_memory_warning());
    }

    #[test]
    fn autosave_lives_under_data_dir() {
        assert!(autosave_path().starts_with(data_dir()));
    }
}
