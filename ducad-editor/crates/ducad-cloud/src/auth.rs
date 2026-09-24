//! Otentikasi OAuth 2.0 & manajemen sesi sisi klien DUCAD.
//!
//! Alur (meniru pola yang sudah terbukti di klien desktop lain milik tim,
//! lihat `docs/adr/0003-identitas-auth-ducad.md`):
//!
//! 1. Klien membuat *ticket* acak 128-bit dan membuka browser ke
//!    `{server}/api/v1/auth/login/{provider}?client=ducad&ticket=…[&port=…]`.
//! 2. Server mengurus seluruh dansa OAuth dengan provider, lalu menyimpan
//!    token pada ticket tersebut.
//! 3. Klien mengambil token lewat DUA jalur sekaligus:
//!    - **Ticket polling HTTPS** ke `/api/v1/auth/ticket/poll` — jalur utama.
//!    - **Callback loopback** `127.0.0.1:{port}` — jalur cepat, opsional.
//!
//! Dua jalur itu bukan kemewahan, masing-masing menutup lubang yang nyata:
//!
//! - **App Sandbox macOS** (lihat `apple/macos/DUCAD.entitlements`) melarang
//!   `TcpListener::bind` tanpa entitlement `com.apple.security.network.server`.
//!   Entitlement itu SENGAJA tidak diminta — reviewer App Store menanyakannya,
//!   dan polling membuatnya tak perlu. Maka kegagalan bind di sini WAJIB
//!   non-fatal; versi awal modul ini langsung `return Err` dan itu membuat
//!   login mati total di build Mac App Store.
//! - **iOS** tidak bisa menjalankan listener loopback sama sekali.
//! - **Apple sign-in** memakai `form_post`, jadi hasilnya mendarat di server,
//!   bukan sebagai redirect ke loopback. Selain itu Safari memblokir `fetch`
//!   lintas-origin dari HTTPS ke `127.0.0.1`, sehingga tanpa polling token
//!   Apple tidak punya jalan pulang ke aplikasi.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use log::{debug, info, warn};

use crate::types::{DucadAccount, OAuthProvider, TokenResponse};

/// Batas waktu keseluruhan satu percobaan login.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(180);
/// Jeda antar permintaan poll ke server.
const POLL_INTERVAL: Duration = Duration::from_millis(1500);
/// Timeout per permintaan HTTP poll.
const POLL_HTTP_TIMEOUT: Duration = Duration::from_secs(5);
/// Setelah token diterima lewat polling, browser mungkin masih mencoba
/// menembak loopback. Layani sebentar supaya tab pengguna tidak berakhir
/// dengan galat koneksi.
const LOOPBACK_DRAIN: Duration = Duration::from_secs(3);

/// Halaman yang ditampilkan di browser setelah callback loopback diterima.
const SUCCESS_HTML: &str = "HTTP/1.1 200 OK\r\n\
     Content-Type: text/html; charset=utf-8\r\n\
     Access-Control-Allow-Origin: *\r\n\
     Connection: close\r\n\r\n\
     <!DOCTYPE html><html><body style='font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;text-align:center;padding:40px;background:#0f172a;color:#f8fafc;'>\
     <h2 style='color:#38bdf8;'>✨ Login DUCAD Berhasil!</h2><p style='color:#94a3b8;'>Anda dapat menutup tab ini dan kembali ke aplikasi DUCAD.</p></body></html>";

/// Memulai alur login: buka browser, lalu tunggu token lewat ticket polling
/// dan/atau callback loopback.
///
/// Receiver yang dikembalikan menghasilkan tepat satu nilai: token, atau
/// pesan galat yang layak ditampilkan ke pengguna.
pub fn start_oauth_flow(
    server_url: &str,
    provider: OAuthProvider,
) -> mpsc::Receiver<Result<TokenResponse, String>> {
    let (tx, rx) = mpsc::channel();
    let server_url = server_url.trim_end_matches('/').to_string();
    let ticket = match generate_ticket() {
        Ok(ticket) => ticket,
        Err(e) => {
            warn!("{}", e);
            let _ = tx.send(Err(e));
            return rx;
        }
    };

    // Listener loopback bersifat best-effort: di App Sandbox macOS dan di iOS
    // ini memang gagal, dan itu bukan galat — polling yang mengambil alih.
    let (listener, port) = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => {
            let port = listener.local_addr().ok().map(|addr| addr.port());
            // Non-blocking supaya loop di bawah bisa menyelang antara
            // accept() dan poll HTTPS tanpa salah satu memblokir yang lain.
            if let Err(e) = listener.set_nonblocking(true) {
                warn!("Gagal menyetel listener non-blocking: {}", e);
            }
            (Some(listener), port)
        }
        Err(e) => {
            info!(
                "Listener loopback tidak tersedia (App Sandbox atau jaringan terbatas): {} \
                 — login dilanjutkan lewat ticket polling HTTPS",
                e
            );
            (None, None)
        }
    };

    let login_url = build_login_url(&server_url, provider, &ticket, port);
    if let Err(e) = open_url(&login_url) {
        // Belum fatal: pengguna masih bisa menyelesaikan login bila browser
        // sempat terbuka, jadi jangan bunuh alurnya di sini.
        warn!("Gagal membuka browser otomatis: {}", e);
    }
    info!("🌐 Membuka URL OAuth {}: {}", provider.label(), login_url);

    thread::spawn(move || {
        info!(
            "🔑 Menunggu otentikasi {} (ticket: {}, port loopback: {:?})",
            provider.label(),
            ticket,
            port
        );

        let poll_url = format!("{}/api/v1/auth/ticket/poll", server_url);
        let agent = ureq::AgentBuilder::new().timeout(POLL_HTTP_TIMEOUT).build();

        let started = Instant::now();
        // `None` = belum pernah poll, jadi putaran pertama langsung menembak
        // tanpa menunggu satu interval. Sengaja BUKAN
        // `Instant::now() - POLL_INTERVAL`: `Sub<Duration> for Instant` panic
        // bila hasilnya lebih awal dari instant terkecil yang bisa diwakili.
        let mut last_poll: Option<Instant> = None;

        while started.elapsed() < LOGIN_TIMEOUT {
            // A. Ticket polling lewat HTTPS — jalur utama, selalu tersedia.
            if last_poll.is_none_or(|t| t.elapsed() >= POLL_INTERVAL) {
                last_poll = Some(Instant::now());
                match poll_ticket(&agent, &poll_url, &ticket) {
                    Ok(Some(token)) => {
                        info!("✅ Token diterima lewat ticket polling");
                        let _ = tx.send(Ok(token));
                        drain_loopback(listener);
                        return;
                    }
                    Ok(None) => {} // masih pending
                    Err(e) => {
                        warn!("❌ Otentikasi gagal di server: {}", e);
                        let _ = tx.send(Err(e));
                        return;
                    }
                }
            }

            // B. Callback loopback — jalur cepat bila OS mengizinkan.
            if let Some(ref listener) = listener {
                match accept_loopback_token(listener) {
                    Ok(Some(token)) => {
                        info!("✅ Token diterima lewat callback loopback");
                        let _ = tx.send(Ok(token));
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        warn!("❌ Payload token dari loopback tidak valid: {}", e);
                        let _ = tx.send(Err(e));
                        return;
                    }
                }
            }

            thread::sleep(Duration::from_millis(200));
        }

        let _ = tx.send(Err("Proses login timeout setelah 3 menit".to_string()));
    });

    rx
}

/// Ticket acak 128-bit dalam bentuk 32 karakter hex.
///
/// Ticket ini adalah kredensial pembawa: siapa pun yang menebaknya sebelum
/// pengguna selesai login bisa menjemput token dengan poll. Karena itu ia
/// WAJIB berasal dari CSPRNG sistem, dan kegagalannya membatalkan login
/// alih-alih jatuh ke sumber acak yang lebih lemah.
fn generate_ticket() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|e| format!("Gagal membuat ticket login dari CSPRNG sistem: {}", e))?;
    Ok(hex::encode(bytes))
}

/// Menyusun URL login. `client=ducad` yang membuat halaman sukses di browser
/// tampil dengan identitas DUCAD, bukan aplikasi lain di server yang sama.
fn build_login_url(
    server_url: &str,
    provider: OAuthProvider,
    ticket: &str,
    port: Option<u16>,
) -> String {
    let mut url = format!(
        "{}/api/v1/auth/login/{}?client=ducad&ticket={}",
        server_url.trim_end_matches('/'),
        provider.path(),
        ticket
    );
    if let Some(port) = port {
        url.push_str(&format!("&port={}", port));
    }
    url
}

/// Satu putaran poll.
///
/// - `Ok(Some(token))` — login selesai.
/// - `Ok(None)` — masih menunggu, atau permintaan poll ini gagal sementara
///   (jaringan putus sesaat bukan alasan membatalkan login).
/// - `Err(msg)` — server menyatakan otentikasi gagal; alur harus berhenti.
fn poll_ticket(
    agent: &ureq::Agent,
    poll_url: &str,
    ticket: &str,
) -> Result<Option<TokenResponse>, String> {
    let body = serde_json::json!({ "ticket": ticket });
    let response = match agent.post(poll_url).send_json(body) {
        Ok(response) => response,
        Err(e) => {
            debug!("Poll ticket belum berhasil: {}", e);
            return Ok(None);
        }
    };

    let json: serde_json::Value = match response.into_json() {
        Ok(json) => json,
        Err(e) => {
            debug!("Respons poll bukan JSON yang valid: {}", e);
            return Ok(None);
        }
    };

    parse_poll_response(&json)
}

/// Menafsirkan body respons poll. Dipisah dari I/O agar bisa diuji tanpa
/// server. Server membungkus payload dalam `data`; bentuk tanpa bungkus juga
/// diterima supaya klien tidak pecah kalau pembungkusnya berubah.
fn parse_poll_response(json: &serde_json::Value) -> Result<Option<TokenResponse>, String> {
    let data = json.get("data").unwrap_or(json);

    match data.get("status").and_then(|s| s.as_str()) {
        Some("completed") => {
            let token_value = data
                .get("token")
                .ok_or_else(|| "Server melaporkan login selesai tanpa token".to_string())?;
            serde_json::from_value::<TokenResponse>(token_value.clone())
                .map(Some)
                .map_err(|e| format!("Payload token dari server tidak valid: {}", e))
        }
        Some("error") => Err(data
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("Otentikasi gagal di server")
            .to_string()),
        // "pending", status tak dikenal, atau respons tanpa status: tunggu.
        _ => Ok(None),
    }
}

/// Memeriksa satu koneksi loopback yang menunggu, bila ada.
///
/// Listener-nya non-blocking, jadi "belum ada koneksi" pun muncul sebagai
/// `Err` dari `accept()` dan diperlakukan sebagai `Ok(None)`.
fn accept_loopback_token(listener: &TcpListener) -> Result<Option<TokenResponse>, String> {
    let Ok((mut stream, _)) = listener.accept() else {
        return Ok(None);
    };

    // Koneksi sudah ada: mulai dari sini blocking dengan batas waktu, karena
    // request browser bisa datang terpotong beberapa paket.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));

    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return Ok(None),
    };
    let request = String::from_utf8_lossy(&buf[..n]);

    // Preflight CORS: jawab, lalu tunggu permintaan sebenarnya.
    if request.starts_with("OPTIONS") {
        let cors = "HTTP/1.1 204 No Content\r\n\
                    Access-Control-Allow-Origin: *\r\n\
                    Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
                    Access-Control-Allow-Headers: *\r\n\
                    Connection: close\r\n\r\n";
        let _ = stream.write_all(cors.as_bytes());
        let _ = stream.flush();
        return Ok(None);
    }

    let token_json = extract_token_payload(&request);

    // Jawab browser lebih dulu, apa pun hasil parsing-nya, supaya tab
    // pengguna tidak menggantung.
    let _ = stream.write_all(SUCCESS_HTML.as_bytes());
    let _ = stream.flush();

    match token_json {
        Some(json) => serde_json::from_str::<TokenResponse>(&json)
            .map(Some)
            .map_err(|e| format!("Payload token tidak valid: {}", e)),
        None => Ok(None),
    }
}

/// Mengambil JSON token dari request loopback: body untuk `POST`, query
/// `?token=` untuk `GET` (jalur fallback yang dipakai browser bila `fetch`
/// ke loopback diblokir).
fn extract_token_payload(request: &str) -> Option<String> {
    if request.starts_with("POST") {
        return request
            .split("\r\n\r\n")
            .nth(1)
            .map(|body| body.trim().to_string())
            .filter(|body| !body.is_empty());
    }

    if request.starts_with("GET") {
        let pos = request.find("token=")?;
        let query = &request[pos + "token=".len()..];
        let end = query.find(' ').unwrap_or(query.len());
        return Some(url_decode(&query[..end]));
    }

    None
}

/// Melayani koneksi loopback yang tersisa setelah token didapat lewat
/// polling, supaya browser menampilkan halaman sukses alih-alih galat.
fn drain_loopback(listener: Option<TcpListener>) {
    let Some(listener) = listener else {
        return;
    };
    let started = Instant::now();
    while started.elapsed() < LOOPBACK_DRAIN {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.write_all(SUCCESS_HTML.as_bytes());
            let _ = stream.flush();
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Mendekode string URL-encoded (`%XX` dan `+`).
///
/// Byte `%XX` yang tidak valid dibiarkan apa adanya alih-alih dibuang, agar
/// payload yang cacat terlihat saat parsing JSON, bukan berubah diam-diam.
fn url_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut decoded = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(val) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
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

/// Membuka URL di browser default sistem.
///
/// Satu matriks platform untuk seluruh aplikasi: setiap target yang hilang
/// dari sini berarti login yang menggantung tanpa pesan galat, karena
/// browser tak pernah terbuka sementara poller menunggu sampai timeout.
pub fn open_url(url: &str) -> Result<(), String> {
    debug!("Membuka URL: {}", url);
    open_url_impl(url)
}

#[cfg(target_os = "macos")]
fn open_url_impl(url: &str) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn open_url_impl(url: &str) -> Result<(), String> {
    // Argumen "" kosong adalah judul window yang diharapkan `start`; tanpa
    // itu URL dalam tanda kutip dianggap judul dan tidak ada yang terbuka.
    std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn open_url_impl(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// iOS tidak punya proses anak untuk di-`spawn`, jadi satu-satunya jalur
/// yang didukung adalah `-[UIApplication openURL:options:completionHandler:]`,
/// yang WAJIB dipanggil dari main thread. Loop update egui berjalan di main
/// thread pada iOS, sehingga jalur tombol login memenuhi syarat itu; panggilan
/// dari thread lain ditolak dengan galat alih-alih mengambil risiko assertion
/// UIKit yang mematikan aplikasi.
#[cfg(target_os = "ios")]
fn open_url_impl(url: &str) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2_foundation::{NSDictionary, NSString, NSURL};
    use objc2_ui_kit::UIApplication;

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "openURL harus dipanggil dari main thread".to_string())?;

    let ns_string = NSString::from_str(url);
    // Tanpa `unsafe`: di objc2-foundation 0.3 `URLWithString` sudah aman
    // (versi 0.2 belum, jadi contoh yang lebih lama memakai blok unsafe di
    // sini — menyalinnya menghasilkan peringatan `unused_unsafe` yang hanya
    // muncul saat kompilasi target iOS).
    let ns_url =
        NSURL::URLWithString(&ns_string).ok_or_else(|| format!("URL tidak valid: {}", url))?;

    let app = UIApplication::sharedApplication(mtm);
    let options = NSDictionary::new();
    unsafe { app.openURL_options_completionHandler(&ns_url, &options, None) };

    Ok(())
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux",
    target_os = "ios"
)))]
fn open_url_impl(_url: &str) -> Result<(), String> {
    Err("Platform ini tidak mendukung pembukaan browser otomatis".to_string())
}

/// Tier yang dipakai bila server tidak menyebutkan `license_tier`.
///
/// Nilainya sama dengan yang dulu di-hardcode di sini, supaya sesi terhadap
/// server lama tetap tampil seperti sebelumnya alih-alih mendadak turun tier.
const FALLBACK_LICENSE_TIER: &str = "Pro";

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
        // Tier datang dari server (`ducad-server` menyimpannya per akun);
        // sebelumnya nilai ini di-hardcode, sehingga setiap pengguna tampil
        // sebagai Pro apa pun yang tercatat di sisi server.
        license_tier: resp
            .user
            .license_tier
            .clone()
            .filter(|tier| !tier.trim().is_empty())
            .unwrap_or_else(|| FALLBACK_LICENSE_TIER.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn token_json() -> serde_json::Value {
        serde_json::json!({
            "access_token": "akses",
            "refresh_token": "segar",
            "expires_in": 3600,
            "user": {
                "id": "usr_1",
                "email": "teknisi@ducad.app"
            }
        })
    }

    #[test]
    fn test_ticket_is_32_hex_chars() {
        let ticket = generate_ticket().expect("CSPRNG sistem harus tersedia di host tes");
        assert_eq!(ticket.len(), 32);
        assert!(ticket.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(
            ticket,
            generate_ticket().expect("CSPRNG sistem harus tersedia di host tes")
        );
    }

    #[test]
    fn test_login_url_carries_ducad_identity() {
        let url = build_login_url(
            "https://api.ducad.app/",
            OAuthProvider::Apple,
            "abc123",
            Some(49152),
        );
        assert_eq!(
            url,
            "https://api.ducad.app/api/v1/auth/login/apple?client=ducad&ticket=abc123&port=49152"
        );
    }

    #[test]
    fn test_login_url_without_loopback_port() {
        // Jalur App Sandbox macOS / iOS: tanpa port, server tahu klien akan
        // mengambil token lewat polling.
        let url = build_login_url(
            "https://api.ducad.app",
            OAuthProvider::Google,
            "def456",
            None,
        );
        assert_eq!(
            url,
            "https://api.ducad.app/api/v1/auth/login/google?client=ducad&ticket=def456"
        );
        assert!(!url.contains("port="));
    }

    #[test]
    fn test_parse_poll_completed() {
        let json = serde_json::json!({
            "success": true,
            "data": { "status": "completed", "token": token_json() }
        });
        let token = parse_poll_response(&json)
            .expect("status completed harus berhasil")
            .expect("token harus ada");
        assert_eq!(token.access_token, "akses");
        assert_eq!(token.user.email, "teknisi@ducad.app");
    }

    #[test]
    fn test_parse_poll_completed_without_envelope() {
        let json = serde_json::json!({ "status": "completed", "token": token_json() });
        assert!(parse_poll_response(&json)
            .expect("harus berhasil")
            .is_some());
    }

    #[test]
    fn test_parse_poll_pending_keeps_waiting() {
        let json = serde_json::json!({ "data": { "status": "pending" } });
        assert!(parse_poll_response(&json).expect("harus Ok").is_none());
    }

    #[test]
    fn test_parse_poll_error_is_forwarded() {
        let json = serde_json::json!({
            "data": { "status": "error", "error": "Sign in with Apple is not configured" }
        });
        let err = parse_poll_response(&json).expect_err("harus Err");
        assert_eq!(err, "Sign in with Apple is not configured");
    }

    #[test]
    fn test_parse_poll_completed_without_token_is_error() {
        let json = serde_json::json!({ "data": { "status": "completed" } });
        assert!(parse_poll_response(&json).is_err());
    }

    #[test]
    fn test_extract_token_payload_from_post_body() {
        let request = format!(
            "POST /callback HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n{}",
            token_json()
        );
        let body = extract_token_payload(&request).expect("body harus terambil");
        assert!(serde_json::from_str::<TokenResponse>(&body).is_ok());
    }

    #[test]
    fn test_extract_token_payload_from_get_query() {
        let request = "GET /callback?token=%7B%22access_token%22%3A%22a%22%7D HTTP/1.1\r\n\r\n";
        let payload = extract_token_payload(request).expect("query harus terambil");
        assert_eq!(payload, r#"{"access_token":"a"}"#);
    }

    #[test]
    fn test_extract_token_payload_ignores_other_methods() {
        assert!(extract_token_payload("HEAD /callback HTTP/1.1\r\n\r\n").is_none());
        assert!(extract_token_payload("GET /callback HTTP/1.1\r\n\r\n").is_none());
    }

    #[test]
    fn test_url_decode_keeps_malformed_escape() {
        assert_eq!(url_decode("a+b"), "a b");
        assert_eq!(url_decode("%7B%7D"), "{}");
        // `%ZZ` bukan hex: dibiarkan agar cacatnya terlihat saat parse JSON.
        assert_eq!(url_decode("%ZZ"), "%ZZ");
    }

    #[test]
    fn test_token_to_account_conversion() {
        let resp: TokenResponse =
            serde_json::from_value(token_json()).expect("fixture harus valid");
        let account = token_to_account(&resp);
        assert_eq!(account.user_id, "usr_1");
        assert_eq!(account.email, "teknisi@ducad.app");
        // Fixture tidak menyebut license_tier: pakai nilai bawaan.
        assert_eq!(account.license_tier, "Pro");
        assert!(!account.is_token_expired());
    }

    #[test]
    fn test_token_to_account_memakai_tier_dari_server() {
        let mut json = token_json();
        json["user"]["license_tier"] = serde_json::json!("Free");
        let resp: TokenResponse = serde_json::from_value(json).expect("fixture harus valid");
        assert_eq!(token_to_account(&resp).license_tier, "Free");
    }

    #[test]
    fn test_token_to_account_mengabaikan_tier_kosong() {
        // Server yang mengirim string kosong tidak boleh membuat drawer akun
        // menampilkan "Ducad  Tier".
        let mut json = token_json();
        json["user"]["license_tier"] = serde_json::json!("   ");
        let resp: TokenResponse = serde_json::from_value(json).expect("fixture harus valid");
        assert_eq!(token_to_account(&resp).license_tier, "Pro");
    }
}
