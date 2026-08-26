//! Modul Otentikasi OAuth 2.0 Loopback & Manajemen Sesi Sisi Klien Ducad.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use chrono::Utc;
use log::{info, warn};

use crate::types::{DucadAccount, OAuthProvider, TokenResponse};

/// Membuka browser pengguna dan mendengarkan callback login di loopback lokal `127.0.0.1:{port}`.
pub fn start_oauth_flow(
    server_url: &str,
    provider: OAuthProvider,
) -> mpsc::Receiver<Result<TokenResponse, String>> {
    let (tx, rx) = mpsc::channel();

    // Bind pada port ephemeral yang dialokasikan oleh sistem operasi
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => {
            warn!("Gagal membuka TCP loopback listener: {}", e);
            let url = format!(
                "{}/api/v1/auth/login/{}?client=ducad",
                server_url.trim_end_matches('/'),
                provider.path()
            );
            let _ = open_url(&url);
            let _ = tx.send(Err(format!("Gagal membuka local listener: {}", e)));
            return rx;
        }
    };

    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(_) => 0,
    };

    // Thread background penunggu callback
    thread::spawn(move || {
        info!("🔑 Menunggu callback OAuth CMJCode Server pada 127.0.0.1:{}", port);
        let start_time = std::time::Instant::now();

        while start_time.elapsed() < Duration::from_secs(180) {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut buf = [0u8; 8192];
                let n = match stream.read(&mut buf) {
                    Ok(n) if n > 0 => n,
                    _ => continue,
                };

                let request_str = String::from_utf8_lossy(&buf[..n]);

                // Handle CORS preflight
                if request_str.starts_with("OPTIONS") {
                    let cors_resp = "HTTP/1.1 204 No Content\r\n\
                                     Access-Control-Allow-Origin: *\r\n\
                                     Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
                                     Access-Control-Allow-Headers: *\r\n\
                                     Connection: close\r\n\r\n";
                    let _ = stream.write_all(cors_resp.as_bytes());
                    let _ = stream.flush();
                    continue;
                }

                // Ekstraksi payload token (POST JSON body atau GET query token)
                let token_json_opt = if request_str.starts_with("POST") {
                    request_str.split("\r\n\r\n").nth(1).map(|s| s.trim().to_string())
                } else if request_str.starts_with("GET") {
                    if let Some(pos) = request_str.find("token=") {
                        let query_part = &request_str[pos + 6..];
                        let end_pos = query_part.find(' ').unwrap_or(query_part.len());
                        Some(url_decode(&query_part[..end_pos]))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Kirim respons HTML sukses ke browser
                let http_resp = "HTTP/1.1 200 OK\r\n\
                                 Content-Type: text/html; charset=utf-8\r\n\
                                 Access-Control-Allow-Origin: *\r\n\
                                 Connection: close\r\n\r\n\
                                 <!DOCTYPE html><html><body style='font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;text-align:center;padding:40px;background:#0f172a;color:#f8fafc;'>\
                                 <h2 style='color:#38bdf8;'>✨ Login Ducad Berhasil!</h2><p style='color:#94a3b8;'>Anda dapat menutup tab ini dan kembali ke aplikasi Ducad.</p></body></html>";
                let _ = stream.write_all(http_resp.as_bytes());
                let _ = stream.flush();

                if let Some(json_str) = token_json_opt {
                    match serde_json::from_str::<TokenResponse>(&json_str) {
                        Ok(token_resp) => {
                            info!("✅ Berhasil menerima token otentikasi Ducad via loopback HTTP");
                            let _ = tx.send(Ok(token_resp));
                            return;
                        }
                        Err(e) => {
                            warn!("❌ Gagal mem-parse TokenResponse dari callback: {}", e);
                            let _ = tx.send(Err(format!("Payload token tidak valid: {}", e)));
                            return;
                        }
                    }
                }
            }
        }

        let _ = tx.send(Err("Proses login timeout setelah 3 menit".to_string()));
    });

    let login_url = format!(
        "{}/api/v1/auth/login/{}?client=ducad&port={}",
        server_url.trim_end_matches('/'),
        provider.path(),
        port
    );

    if let Err(e) = open_url(&login_url) {
        warn!("Gagal membuka browser otomatis: {}", e);
    }

    info!("🌐 Membuka URL OAuth: {}", login_url);
    rx
}

/// Helper untuk mendekode string URL-encoded
fn url_decode(input: &str) -> String {
    let mut decoded = String::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                decoded.push(val as char);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            decoded.push(' ');
            i += 1;
            continue;
        }
        decoded.push(bytes[i] as char);
        i += 1;
    }
    decoded
}

/// Membuka URL di browser default sistem operasi
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("Platform tidak mendukung open browser otomatis".to_string())
    }
}

/// Mengonversi TokenResponse dari server menjadi model DucadAccount lokal
pub fn token_to_account(resp: &TokenResponse) -> DucadAccount {
    let expires_at = Utc::now().timestamp() + resp.expires_in;
    DucadAccount {
        user_id: resp.user.id.clone(),
        email: resp.user.email.clone(),
        display_name: resp.user.display_name.clone(),
        avatar_url: resp.user.avatar_url.clone(),
        username: resp.user.username.clone(),
        phone: resp.user.phone.clone(),
        access_token: resp.access_token.clone(),
        refresh_token: resp.refresh_token.clone(),
        token_expires_at: expires_at,
        license_tier: "Pro".to_string(),
    }
}

/// Mengambil path file penyimpanan sesi akun lokal (`~/.ducad/session.json`)
pub fn session_file_path() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ducad");
    let _ = fs::create_dir_all(&dir);
    dir.join("session.json")
}

/// Menyimpan data akun ke file sesi lokal
pub fn save_account(account: &DucadAccount) -> anyhow::Result<()> {
    let path = session_file_path();
    let json = serde_json::to_string_pretty(account)?;
    fs::write(path, json)?;
    Ok(())
}

/// Memuat sesi akun pengguna dari file lokal jika ada
pub fn load_account() -> Option<DucadAccount> {
    let path = session_file_path();
    if !path.exists() {
        return None;
    }
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str::<DucadAccount>(&data).ok()
}

/// Menghapus sesi akun (Logout)
pub fn clear_account() -> anyhow::Result<()> {
    let path = session_file_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}
