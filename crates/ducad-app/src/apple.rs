//! Modul integrasi platform Apple native (macOS & iOS) menggunakan `objc2` dan `block2`.
//!
//! Menyediakan helper aman dan sound untuk integrasi sistem operasi Apple:
//! - Resolusi folder `Documents` dan `Application Support` sandboxed via `objc2-foundation` (NSFileManager).
//! - Identifikasi platform Apple (macOS vs iOS).

#![allow(dead_code)]

use std::path::PathBuf;

use objc2_foundation::{
    NSFileManager, NSSearchPathDirectory, NSSearchPathDomainMask,
};

/// Mengambil path direktori `Documents` sandboxed aplikasi menggunakan `NSFileManager` dari Foundation.
///
/// Pada iOS dan macOS dengan sandbox, direktori ini adalah lokasi standar tempat dokumen pengguna disimpan.
/// Jika pemanggilan Objective-C gagal atau mengembalikan array kosong, fungsi ini melakukan fallback ke `$HOME/Documents`.
pub fn apple_documents_directory() -> PathBuf {
    let file_manager = NSFileManager::defaultManager();
    let urls = file_manager.URLsForDirectory_inDomains(
        NSSearchPathDirectory::DocumentDirectory,
        NSSearchPathDomainMask::UserDomainMask,
    );

    if let Some(first_url) = urls.firstObject() {
        if let Some(path_str) = first_url.path() {
            return PathBuf::from(path_str.to_string());
        }
    }

    // Fallback bila NSFileManager tidak menghasilkan URL
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Documents")
}

/// Mengambil path direktori `Application Support` aplikasi menggunakan `NSFileManager`.
pub fn apple_app_support_directory() -> PathBuf {
    let file_manager = NSFileManager::defaultManager();
    let urls = file_manager.URLsForDirectory_inDomains(
        NSSearchPathDirectory::ApplicationSupportDirectory,
        NSSearchPathDomainMask::UserDomainMask,
    );

    if let Some(first_url) = urls.firstObject() {
        if let Some(path_str) = first_url.path() {
            return PathBuf::from(path_str.to_string());
        }
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Library").join("Application Support")
}

/// Mengembalikan nama platform Apple yang sedang aktif.
pub fn apple_platform_name() -> &'static str {
    #[cfg(target_os = "ios")]
    {
        "iOS"
    }
    #[cfg(target_os = "macos")]
    {
        "macOS"
    }
    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    {
        "Apple Platform"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_documents_directory_not_empty() {
        let docs = apple_documents_directory();
        assert!(!docs.as_os_str().is_empty());
        assert!(docs.to_str().unwrap().contains("Documents"));
    }

    #[test]
    fn test_apple_app_support_directory_not_empty() {
        let app_support = apple_app_support_directory();
        assert!(!app_support.as_os_str().is_empty());
        assert!(app_support.to_str().unwrap().contains("Application Support"));
    }

    #[test]
    fn test_apple_platform_name() {
        let name = apple_platform_name();
        assert!(!name.is_empty());
        #[cfg(target_os = "macos")]
        assert_eq!(name, "macOS");
        #[cfg(target_os = "ios")]
        assert_eq!(name, "iOS");
    }
}
