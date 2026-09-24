//! API Client untuk berkomunikasi dengan server auth & sync DUCAD.

use std::fs;
use std::path::Path;

/// Server auth/sync DUCAD produksi. Instance ini punya kredensial OAuth
/// milik DUCAD sendiri (Google Client "DUCAD", GitHub OAuth App "DUCAD",
/// Services ID Apple `id.ducad.studio.signin`) sehingga layar consent yang
/// dilihat pengguna menyebut DUCAD — bukan aplikasi lain yang memakai basis
/// kode server yang sama. Lihat `docs/AUTH_SERVER.md`.
pub const DEFAULT_SERVER_URL: &str = "https://api.ducad.app";

/// Nama env var yang diperiksa berurutan untuk menemukan URL server.
///
/// `CMJCODE_SERVER_URL` dipertahankan di urutan kedua sebagai fallback
/// kompatibilitas: itu nama lama dari era ketika DUCAD menumpang SSO
/// CMJCode, dan masih terpakai di setup pengembangan lokal. Yang baru
/// (`DUCAD_SERVER_URL`) menang bila keduanya ada.
const SERVER_URL_ENV_KEYS: [&str; 3] =
    ["DUCAD_SERVER_URL", "CMJCODE_SERVER_URL", "SERVER_BASE_URL"];

/// Mendeteksi URL server secara otomatis dari environment variable atau file `.env`.
pub fn detect_server_url() -> String {
    // 1. Cek environment variables
    for key in SERVER_URL_ENV_KEYS {
        if let Ok(url) = std::env::var(key) {
            let url = url.trim();
            if !url.is_empty() {
                return url.trim_end_matches('/').to_string();
            }
        }
    }

    // 2. Cek file .env di direktori saat ini
    for key in SERVER_URL_ENV_KEYS {
        if let Some(url) = read_env_var_from_file(".env", key) {
            return url.trim_end_matches('/').to_string();
        }
    }

    DEFAULT_SERVER_URL.to_string()
}

fn read_env_var_from_file(path: impl AsRef<Path>, key: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    parse_env_var(&content, key)
}

/// Mencari `key=value` di dalam isi berkas `.env`, melewati komentar dan
/// baris kosong. Dipisah dari I/O supaya bisa diuji tanpa menyentuh disk.
fn parse_env_var(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                let val = v.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct CloudClient {
    pub server_url: String,
}

impl Default for CloudClient {
    fn default() -> Self {
        Self {
            server_url: detect_server_url(),
        }
    }
}

impl CloudClient {
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_var_skips_comments_and_blanks() {
        let content = "\n# DUCAD_SERVER_URL=https://jangan-dipakai.example\n\nDUCAD_SERVER_URL=https://api.ducad.app\n";
        assert_eq!(
            parse_env_var(content, "DUCAD_SERVER_URL").as_deref(),
            Some("https://api.ducad.app")
        );
    }

    #[test]
    fn test_parse_env_var_strips_quotes() {
        let content = "DUCAD_SERVER_URL=\"https://staging.ducad.app\"\n";
        assert_eq!(
            parse_env_var(content, "DUCAD_SERVER_URL").as_deref(),
            Some("https://staging.ducad.app")
        );
    }

    #[test]
    fn test_parse_env_var_accepts_legacy_key() {
        // Setup lokal lama masih memakai nama CMJCODE_*; harus tetap terbaca.
        let content = "CMJCODE_SERVER_URL=http://127.0.0.1:3000\n";
        assert_eq!(
            parse_env_var(content, "CMJCODE_SERVER_URL").as_deref(),
            Some("http://127.0.0.1:3000")
        );
        assert!(parse_env_var(content, "DUCAD_SERVER_URL").is_none());
    }

    #[test]
    fn test_env_key_precedence_puts_ducad_first() {
        assert_eq!(SERVER_URL_ENV_KEYS[0], "DUCAD_SERVER_URL");
        assert!(SERVER_URL_ENV_KEYS.contains(&"CMJCODE_SERVER_URL"));
    }

    #[test]
    fn test_default_server_url_is_https() {
        // Apple menolak Return URL non-HTTPS untuk Sign in with Apple, jadi
        // default yang bocor ke http:// akan mematikan login Apple diam-diam.
        assert!(DEFAULT_SERVER_URL.starts_with("https://"));
    }
}
