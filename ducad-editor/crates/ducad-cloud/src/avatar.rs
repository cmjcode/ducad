//! Foto profil akun login (Google/GitHub) untuk avatar di header dan popup akun.
//!
//! URL foto datang dari server DUCAD (`DucadAccount::avatar_url`) dan
//! menunjuk ke CDN penyedia login. Permintaan ke CDN itu sengaja TIDAK
//! membawa token DUCAD. Hasil unduhan di-cache di `~/.ducad/avatar/` agar
//! tidak diunduh ulang setiap kali aplikasi dibuka; nama berkasnya diturunkan
//! dari URL, jadi foto yang berganti di sisi penyedia otomatis diunduh ulang.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use log::debug;

use crate::auth::token_to_account;
use crate::types::{DucadAccount, TokenResponse, UserResponse};

/// Batas ukuran foto yang diterima. Avatar 128 px jauh di bawah ini; batas
/// mencegah respons aneh/berbahaya memakan memori.
const MAX_AVATAR_BYTES: u64 = 2 * 1024 * 1024;

/// Sisi foto yang diminta ke penyedia (px). Avatar terbesar 40 pt, jadi 128
/// cukup tajam di layar Retina @3x.
const AVATAR_PIXELS: u32 = 128;

const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Menambahkan parameter ukuran untuk host penyedia yang dikenal, supaya yang
/// diunduh thumbnail kecil (JPEG/PNG), bukan gambar asli berukuran penuh.
pub fn sized_avatar_url(url: &str) -> String {
    if url.contains("googleusercontent.com") {
        // Foto Google berakhiran `=s96-c` (atau tanpa sufiks); ganti/tambah.
        let base = match url.rfind('=') {
            Some(i) if url[i + 1..].starts_with('s') && !url[i..].contains('/') => &url[..i],
            _ => url,
        };
        return format!("{base}=s{AVATAR_PIXELS}-c");
    }
    if url.contains("avatars.githubusercontent.com") && !url.contains("s=") {
        let sep = if url.contains('?') { '&' } else { '?' };
        return format!("{url}{sep}s={AVATAR_PIXELS}");
    }
    url.to_string()
}

/// Folder cache avatar (`~/.ducad/avatar`), sejajar dengan `session.json`.
pub fn avatar_cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ducad")
        .join("avatar")
}

/// Berkas cache untuk satu URL foto. Hash FNV-1a ditulis tangan agar nama
/// berkas stabil antar versi Rust (`DefaultHasher` tidak menjaminnya).
fn cache_file(dir: &Path, url: &str) -> PathBuf {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    dir.join(format!("avatar-{hash:016x}.img"))
}

/// Mengambil bytes foto profil: dari cache bila ada, selain itu dari jaringan
/// (lalu disimpan ke cache). Memblokir — panggil dari thread latar.
pub fn fetch_avatar(url: &str) -> Result<Vec<u8>, String> {
    fetch_avatar_in(&avatar_cache_dir(), url)
}

/// https wajib, kecuali server DUCAD lokal saat pengembangan
/// (`http://localhost`/`http://127.0.0.1`) yang menyajikan foto unggahan.
fn is_allowed_avatar_url(url: &str) -> bool {
    url.starts_with("https://")
        || ["http://localhost:", "http://localhost/", "http://127.0.0.1:", "http://127.0.0.1/"]
            .iter()
            .any(|prefix| url.starts_with(prefix))
}

fn fetch_avatar_in(dir: &Path, url: &str) -> Result<Vec<u8>, String> {
    if !is_allowed_avatar_url(url) {
        return Err("URL foto profil harus https".to_string());
    }
    let path = cache_file(dir, url);
    if let Ok(bytes) = fs::read(&path) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }

    let agent = ureq::AgentBuilder::new().timeout(FETCH_TIMEOUT).build();
    let response = agent
        .get(&sized_avatar_url(url))
        .call()
        .map_err(|e| format!("Gagal mengunduh foto profil: {e}"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_AVATAR_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Gagal membaca foto profil: {e}"))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_AVATAR_BYTES {
        return Err("Ukuran foto profil tidak wajar".to_string());
    }

    // Cache gagal ditulis bukan alasan menolak foto yang sudah terunduh.
    if fs::create_dir_all(dir).and_then(|_| fs::write(&path, &bytes)).is_err() {
        debug!("Foto profil tidak bisa di-cache di {}", path.display());
    }
    Ok(bytes)
}

/// Mengunggah foto profil (PNG/JPEG yang sudah diperkecil) ke server DUCAD
/// lewat `PUT /api/v1/users/me/avatar`. Memblokir — panggil dari thread latar.
///
/// Token akses berumur pendek (60 menit) dan klien belum memperbaruinya di
/// latar, jadi bila server menjawab 401 token diperbarui sekali lewat
/// `/api/v1/auth/refresh` lalu unggahan diulang. Hasilnya akun lengkap yang
/// baru (token baru bila diperbarui + `avatar_url` baru) untuk disimpan
/// pemanggil ke sesi lokal.
pub fn upload_avatar(
    server_url: &str,
    account: &DucadAccount,
    image: &[u8],
) -> Result<DucadAccount, String> {
    let agent = ureq::AgentBuilder::new().timeout(UPLOAD_TIMEOUT).build();
    let base = server_url.trim_end_matches('/');
    let mut account = account.clone();

    let user = match put_avatar(&agent, base, &account.access_token, image) {
        Err(UploadError::Unauthorized) => {
            account = refresh_session(&agent, base, &account.refresh_token)?;
            put_avatar(&agent, base, &account.access_token, image).map_err(|e| e.message())?
        }
        other => other.map_err(|e| e.message())?,
    };

    account.display_name = user.display_name.or(account.display_name);
    account.avatar_url = user.avatar_url;
    if let Some(tier) = user.license_tier {
        account.license_tier = tier;
    }
    Ok(account)
}

const UPLOAD_TIMEOUT: Duration = Duration::from_secs(30);

enum UploadError {
    Unauthorized,
    Other(String),
}

impl UploadError {
    fn message(self) -> String {
        match self {
            UploadError::Unauthorized => "Sesi login berakhir. Silakan masuk lagi.".to_string(),
            UploadError::Other(msg) => msg,
        }
    }
}

fn put_avatar(
    agent: &ureq::Agent,
    base: &str,
    access_token: &str,
    image: &[u8],
) -> Result<UserResponse, UploadError> {
    let content_type = if image.starts_with(b"\x89PNG") { "image/png" } else { "image/jpeg" };
    let response = agent
        .put(&format!("{base}/api/v1/users/me/avatar"))
        .set("Authorization", &format!("Bearer {access_token}"))
        .set("Content-Type", content_type)
        .send_bytes(image);
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(401, _)) => return Err(UploadError::Unauthorized),
        Err(ureq::Error::Status(code, response)) => {
            let msg = server_error_message(response)
                .unwrap_or_else(|| format!("Server menolak foto profil (HTTP {code})"));
            return Err(UploadError::Other(msg));
        }
        Err(e) => return Err(UploadError::Other(format!("Gagal mengunggah foto profil: {e}"))),
    };
    let json: serde_json::Value = response
        .into_json()
        .map_err(|e| UploadError::Other(format!("Balasan server tidak valid: {e}")))?;
    let data = json.get("data").unwrap_or(&json);
    serde_json::from_value::<UserResponse>(data.clone())
        .map_err(|e| UploadError::Other(format!("Profil dari server tidak valid: {e}")))
}

pub(crate) fn refresh_session(
    agent: &ureq::Agent,
    base: &str,
    refresh_token: &str,
) -> Result<DucadAccount, String> {
    let response = agent
        .post(&format!("{base}/api/v1/auth/refresh"))
        .send_json(serde_json::json!({ "refresh_token": refresh_token }))
        .map_err(|_| "Sesi login berakhir. Silakan masuk lagi.".to_string())?;
    let json: serde_json::Value = response
        .into_json()
        .map_err(|e| format!("Balasan refresh tidak valid: {e}"))?;
    let data = json.get("data").unwrap_or(&json);
    let token = serde_json::from_value::<TokenResponse>(data.clone())
        .map_err(|e| format!("Token baru dari server tidak valid: {e}"))?;
    Ok(token_to_account(&token))
}

/// Pesan `{"success": false, "error": "..."}` dari server, bila ada.
fn server_error_message(response: ureq::Response) -> Option<String> {
    let json: serde_json::Value = response.into_json().ok()?;
    json.get("error")?.as_str().map(str::to_owned)
}

/// Menghapus semua foto profil ter-cache (dipanggil saat logout).
pub fn clear_avatar_cache() {
    let dir = avatar_cache_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("avatar-") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn google_size_suffix_replaced_or_added() {
        assert_eq!(
            sized_avatar_url("https://lh3.googleusercontent.com/a/ABC=s96-c"),
            "https://lh3.googleusercontent.com/a/ABC=s128-c"
        );
        assert_eq!(
            sized_avatar_url("https://lh3.googleusercontent.com/a/ABC"),
            "https://lh3.googleusercontent.com/a/ABC=s128-c"
        );
    }

    #[test]
    fn github_size_param_added_once() {
        assert_eq!(
            sized_avatar_url("https://avatars.githubusercontent.com/u/1?v=4"),
            "https://avatars.githubusercontent.com/u/1?v=4&s=128"
        );
        assert_eq!(
            sized_avatar_url("https://avatars.githubusercontent.com/u/1?s=64"),
            "https://avatars.githubusercontent.com/u/1?s=64"
        );
    }

    #[test]
    fn unknown_host_untouched() {
        assert_eq!(sized_avatar_url("https://cdn.example/a.png"), "https://cdn.example/a.png");
    }

    #[test]
    fn local_dev_server_allowed_over_http() {
        assert!(is_allowed_avatar_url("http://localhost:8430/api/v1/users/u/avatar?v=1"));
        assert!(is_allowed_avatar_url("http://127.0.0.1:8430/a"));
        assert!(!is_allowed_avatar_url("http://localhost.evil.com/a"));
        assert!(!is_allowed_avatar_url("http://example.com/a"));
    }

    #[test]
    fn non_https_rejected_without_network() {
        let dir = std::env::temp_dir().join("ducad-avatar-test-http");
        assert!(fetch_avatar_in(&dir, "http://example.com/a.png").is_err());
        assert!(fetch_avatar_in(&dir, "file:///etc/passwd").is_err());
    }

    #[test]
    fn cache_hit_skips_network() {
        let dir = std::env::temp_dir().join(format!("ducad-avatar-test-{}", std::process::id()));
        let url = "https://unreachable.invalid/a.png";
        fs::create_dir_all(&dir).unwrap();
        fs::write(cache_file(&dir, url), b"PNGDATA").unwrap();
        assert_eq!(fetch_avatar_in(&dir, url).unwrap(), b"PNGDATA");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_name_depends_on_url() {
        let dir = Path::new("/x");
        assert_ne!(cache_file(dir, "https://a/1"), cache_file(dir, "https://a/2"));
        assert_eq!(cache_file(dir, "https://a/1"), cache_file(dir, "https://a/1"));
    }

    /// Server tiruan: menjawab koneksi berurutan dengan respons terskrip dan
    /// mengembalikan baris permintaan + header Authorization yang diterima.
    pub(crate) fn scripted_server(responses: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                // Baca header + body sampai lengkap; menutup koneksi selagi
                // klien masih menulis body membuat klien melihat galat jaringan.
                let mut raw = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = stream.read(&mut chunk).unwrap();
                    raw.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&raw).to_string();
                    if let Some(end) = text.find("\r\n\r\n") {
                        let len = text
                            .lines()
                            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                            .unwrap_or(0);
                        if raw.len() >= end + 4 + len {
                            break;
                        }
                    }
                    if n == 0 {
                        break;
                    }
                }
                let req = String::from_utf8_lossy(&raw).to_string();
                let line = req.lines().next().unwrap_or_default().to_string();
                let auth = req
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
                    .unwrap_or("")
                    .to_string();
                seen.push(format!("{line} | {auth}"));
                let reply = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(reply.as_bytes()).unwrap();
            }
            seen
        });
        (url, handle)
    }

    pub(crate) fn test_account() -> DucadAccount {
        DucadAccount {
            user_id: "u-1".into(),
            email: "a@b.c".into(),
            display_name: Some("Nama".into()),
            avatar_url: None,
            username: None,
            phone: None,
            access_token: "lama".into(),
            refresh_token: "segar-lama".into(),
            token_expires_at: 0,
            license_tier: "Pro".into(),
        }
    }

    #[test]
    fn upload_refreshes_token_once_on_401() {
        let user = r#"{"id":"u-1","email":"a@b.c","display_name":"Nama","avatar_url":"https://api.ducad.app/api/v1/users/u-1/avatar?v=7","license_tier":"Pro"}"#;
        let token = format!(
            r#"{{"success":true,"data":{{"access_token":"baru","refresh_token":"segar-baru","expires_in":3600,"user":{user}}}}}"#
        );
        let (url, server) = scripted_server(vec![
            (401, r#"{"success":false,"error":"Token tidak valid"}"#.into()),
            (200, token),
            (200, format!(r#"{{"success":true,"data":{user}}}"#)),
        ]);
        let acc = upload_avatar(&url, &test_account(), b"\x89PNG\r\n\x1a\nxx").unwrap();
        assert_eq!(acc.access_token, "baru");
        assert_eq!(acc.refresh_token, "segar-baru");
        assert_eq!(acc.avatar_url.as_deref(), Some("https://api.ducad.app/api/v1/users/u-1/avatar?v=7"));
        let seen = server.join().unwrap();
        assert!(seen[0].starts_with("PUT /api/v1/users/me/avatar") && seen[0].contains("Bearer lama"));
        assert!(seen[1].starts_with("POST /api/v1/auth/refresh"));
        assert!(seen[2].starts_with("PUT /api/v1/users/me/avatar") && seen[2].contains("Bearer baru"));
    }

    #[test]
    fn upload_reports_server_error_message() {
        let (url, server) = scripted_server(vec![(
            400,
            r#"{"success":false,"error":"Foto profil harus berupa PNG atau JPEG"}"#.into(),
        )]);
        let err = upload_avatar(&url, &test_account(), b"GIF89a").unwrap_err();
        assert_eq!(err, "Foto profil harus berupa PNG atau JPEG");
        server.join().unwrap();
    }
}
