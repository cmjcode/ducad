//! Cloud Sync: berkas `.ducad` dan preferensi antar perangkat, serta
//! penyegaran profil/tier akun. Kontrak HTTP-nya ada di README `ducad-server`
//! ("Cloud Sync").
//!
//! Semua fungsi memblokir — panggil dari thread latar. Token akses berumur
//! pendek; [`CloudSession`] memperbaruinya sekali saat server menjawab 401
//! lalu mengulang permintaan. Pemanggil wajib menyimpan
//! [`CloudSession::into_account`] bila [`CloudSession::account_changed`],
//! karena refresh token lama sudah tidak berlaku setelah diputar.

use std::io::Read;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{DucadAccount, UserResponse};

const TIMEOUT: Duration = Duration::from_secs(60);

/// Batas unduhan klien: jaring pengaman di atas batas server (32 MB bawaan).
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

/// Metadata satu berkas di cloud.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudFile {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
    pub revision: i64,
    pub updated_at: String,
}

/// Isi `GET /api/v1/sync/files`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileList {
    pub files: Vec<CloudFile>,
    pub used_bytes: u64,
    pub quota_bytes: u64,
    pub max_file_bytes: u64,
    pub max_files: u64,
    pub tier: String,
}

/// Isi `GET/PUT /api/v1/sync/prefs`. `revision == 0` = belum pernah disimpan.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemotePrefs {
    pub prefs: Option<Value>,
    pub revision: i64,
    pub updated_at: Option<String>,
}

/// Galat sync yang bisa ditindaklanjuti UI.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncError {
    /// Refresh token juga ditolak: pengguna harus login lagi.
    SessionExpired,
    /// Revisi di cloud berubah sejak terakhir diunduh/diunggah (409).
    Conflict { message: String, current: Option<Box<CloudFile>> },
    /// Berkas melebihi batas satu berkas (413).
    TooLarge(String),
    /// Kuota byte atau jumlah berkas tier ini habis (403).
    QuotaExceeded(String),
    /// Galat jaringan / server lain.
    Other(String),
}

impl SyncError {
    pub fn message(&self) -> String {
        match self {
            SyncError::SessionExpired => "Sesi login berakhir. Silakan masuk lagi.".to_string(),
            SyncError::Conflict { message, .. }
            | SyncError::TooLarge(message)
            | SyncError::QuotaExceeded(message)
            | SyncError::Other(message) => message.clone(),
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

/// Galat satu percobaan permintaan, sebelum diputuskan apakah perlu refresh.
enum Attempt {
    Unauthorized,
    Failed(SyncError),
}

type AttemptResult<T> = Result<T, Attempt>;

/// Akun + agent HTTP untuk serangkaian permintaan terotentikasi.
pub struct CloudSession {
    agent: ureq::Agent,
    base: String,
    account: DucadAccount,
    changed: bool,
}

impl CloudSession {
    pub fn new(server_url: &str, account: DucadAccount) -> Self {
        Self {
            agent: ureq::AgentBuilder::new().timeout(TIMEOUT).build(),
            base: server_url.trim_end_matches('/').to_string(),
            account,
            changed: false,
        }
    }

    pub fn account(&self) -> &DucadAccount {
        &self.account
    }

    /// Token diperbarui atau profil/tier berubah: simpan akun ke sesi lokal.
    pub fn account_changed(&self) -> bool {
        self.changed
    }

    pub fn into_account(self) -> DucadAccount {
        self.account
    }

    /// Jalankan `f` dengan token akses; pada 401 perbarui token sekali lalu
    /// ulangi.
    fn authed<T>(
        &mut self,
        f: impl Fn(&ureq::Agent, &str, &str) -> AttemptResult<T>,
    ) -> Result<T, SyncError> {
        match f(&self.agent, &self.base, &self.account.access_token) {
            Err(Attempt::Unauthorized) => {
                let renewed =
                    crate::avatar::refresh_session(&self.agent, &self.base, &self.account.refresh_token)
                        .map_err(|_| SyncError::SessionExpired)?;
                self.account = renewed;
                self.changed = true;
                match f(&self.agent, &self.base, &self.account.access_token) {
                    Err(Attempt::Unauthorized) => Err(SyncError::SessionExpired),
                    Err(Attempt::Failed(e)) => Err(e),
                    Ok(v) => Ok(v),
                }
            }
            Err(Attempt::Failed(e)) => Err(e),
            Ok(v) => Ok(v),
        }
    }

    /// `GET /api/v1/users/me`: perbarui profil dan tier lisensi (mis. setelah
    /// admin menaikkan akun ke Pro).
    pub fn refresh_profile(&mut self) -> Result<(), SyncError> {
        let user: UserResponse = self.authed(|agent, base, token| {
            let resp = send(agent.get(&format!("{base}/api/v1/users/me")), token, Body::Empty)?;
            json_data(resp)
        })?;
        let acc = &mut self.account;
        let before = (acc.display_name.clone(), acc.avatar_url.clone(), acc.license_tier.clone());
        acc.display_name = user.display_name.or(acc.display_name.take());
        acc.avatar_url = user.avatar_url.or(acc.avatar_url.take());
        if let Some(tier) = user.license_tier.filter(|t| !t.trim().is_empty()) {
            acc.license_tier = tier;
        }
        if before != (acc.display_name.clone(), acc.avatar_url.clone(), acc.license_tier.clone()) {
            self.changed = true;
        }
        Ok(())
    }

    pub fn list_files(&mut self) -> Result<FileList, SyncError> {
        self.authed(|agent, base, token| {
            let resp = send(agent.get(&format!("{base}/api/v1/sync/files")), token, Body::Empty)?;
            json_data(resp)
        })
    }

    /// Isi berkas `id`.
    pub fn download_file(&mut self, id: &str) -> Result<Vec<u8>, SyncError> {
        self.authed(|agent, base, token| {
            let url = format!("{base}/api/v1/sync/files/{}", path_segment(id));
            let resp = send(agent.get(&url), token, Body::Empty)?;
            let mut bytes = Vec::new();
            resp.into_reader()
                .take(MAX_DOWNLOAD_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| failed(format!("Gagal mengunduh berkas: {e}")))?;
            if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
                return Err(failed("Berkas dari cloud terlalu besar".to_string()));
            }
            Ok(bytes)
        })
    }

    /// Unggah `bytes` sebagai `name`. `base_revision`: 0 = berkas baru (gagal
    /// bila sudah ada), n = revisi yang terakhir dilihat perangkat ini.
    /// `force` menimpa tanpa memeriksa revisi.
    pub fn upload_file(
        &mut self,
        name: &str,
        bytes: &[u8],
        base_revision: i64,
        force: bool,
    ) -> Result<CloudFile, SyncError> {
        self.authed(|agent, base, token| {
            let mut req = agent
                .put(&format!("{base}/api/v1/sync/files"))
                .query("name", name)
                .query("base_revision", &base_revision.to_string())
                .set("Content-Type", "application/octet-stream");
            if force {
                req = req.query("force", "true");
            }
            let resp = send(req, token, Body::Bytes(bytes))?;
            json_data(resp)
        })
    }

    pub fn delete_file(&mut self, id: &str) -> Result<(), SyncError> {
        self.authed(|agent, base, token| {
            let url = format!("{base}/api/v1/sync/files/{}", path_segment(id));
            send(agent.delete(&url), token, Body::Empty).map(|_| ())
        })
    }

    pub fn get_prefs(&mut self) -> Result<RemotePrefs, SyncError> {
        self.authed(|agent, base, token| {
            let resp = send(agent.get(&format!("{base}/api/v1/sync/prefs")), token, Body::Empty)?;
            json_data(resp)
        })
    }

    pub fn put_prefs(&mut self, prefs: &Value) -> Result<RemotePrefs, SyncError> {
        self.authed(|agent, base, token| {
            let body = serde_json::json!({ "prefs": prefs });
            let resp = send(agent.put(&format!("{base}/api/v1/sync/prefs")), token, Body::Json(&body))?;
            json_data(resp)
        })
    }
}

fn failed(msg: String) -> Attempt {
    Attempt::Failed(SyncError::Other(msg))
}

/// Id dari server adalah UUID; apa pun selain karakter aman dibuang agar
/// tidak bisa membelokkan path.
fn path_segment(id: &str) -> String {
    id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

/// Isi permintaan untuk [`send`].
enum Body<'a> {
    Empty,
    Bytes(&'a [u8]),
    Json(&'a Value),
}

/// Tempel token, kirim, dan petakan status HTTP ke [`Attempt`].
fn send(req: ureq::Request, token: &str, body: Body<'_>) -> AttemptResult<ureq::Response> {
    let req = req.set("Authorization", &format!("Bearer {token}"));
    let sent = match body {
        Body::Empty => req.call(),
        Body::Bytes(b) => req.send_bytes(b),
        Body::Json(v) => req.send_json(v),
    };
    match sent {
        Ok(resp) => Ok(resp),
        Err(ureq::Error::Status(401, _)) => Err(Attempt::Unauthorized),
        Err(ureq::Error::Status(code, resp)) => Err(Attempt::Failed(status_error(code, resp))),
        Err(e) => Err(failed(format!("Tidak dapat terhubung ke server DUCAD: {e}"))),
    }
}

fn status_error(code: u16, resp: ureq::Response) -> SyncError {
    // Body 413 dari batas body axum berupa teks biasa, bukan JSON.
    let json: Value = resp.into_json().unwrap_or(Value::Null);
    let message = json
        .get("error")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| match code {
            413 => "Berkas terlalu besar untuk Cloud Sync".to_string(),
            _ => format!("Server menolak permintaan (HTTP {code})"),
        });
    match (code, json.get("code").and_then(Value::as_str)) {
        (409, _) | (_, Some("conflict")) => SyncError::Conflict {
            message,
            current: json
                .get("current")
                .and_then(|c| serde_json::from_value(c.clone()).ok())
                .map(Box::new),
        },
        (413, _) | (_, Some("file_too_large")) => SyncError::TooLarge(message),
        (_, Some("quota_exceeded")) => SyncError::QuotaExceeded(message),
        _ => SyncError::Other(message),
    }
}

/// Ambil `data` dari amplop `{"success":true,"data":...}`.
fn json_data<T: for<'de> Deserialize<'de>>(resp: ureq::Response) -> AttemptResult<T> {
    let json: Value = resp
        .into_json()
        .map_err(|e| failed(format!("Balasan server tidak valid: {e}")))?;
    let data = json.get("data").unwrap_or(&json).clone();
    serde_json::from_value(data).map_err(|e| failed(format!("Balasan server tidak dikenali: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avatar::tests::{scripted_server, test_account};

    const FILE: &str = r#"{"id":"f-1","name":"braket.ducad","size":3,"sha256":"ab","revision":2,"updated_at":"2026-10-20T10:00:00Z"}"#;

    #[test]
    fn list_files_parses_envelope() {
        let body = format!(
            r#"{{"success":true,"data":{{"files":[{FILE}],"used_bytes":3,"quota_bytes":100,"max_file_bytes":50,"max_files":20,"tier":"Free"}}}}"#
        );
        let (url, server) = scripted_server(vec![(200, body)]);
        let mut s = CloudSession::new(&url, test_account());
        let list = s.list_files().unwrap();
        assert_eq!(list.files.len(), 1);
        assert_eq!(list.files[0].revision, 2);
        assert_eq!(list.tier, "Free");
        assert!(!s.account_changed());
        let seen = server.join().unwrap();
        assert!(seen[0].starts_with("GET /api/v1/sync/files") && seen[0].contains("Bearer lama"));
    }

    #[test]
    fn upload_sends_name_and_revision_and_maps_conflict() {
        let body = format!(r#"{{"success":false,"error":"Berkas di cloud sudah berubah","code":"conflict","current":{FILE}}}"#);
        let (url, server) = scripted_server(vec![(409, body)]);
        let mut s = CloudSession::new(&url, test_account());
        let err = s.upload_file("braket kiri.ducad", b"abc", 1, false).unwrap_err();
        match err {
            SyncError::Conflict { current, .. } => assert_eq!(current.unwrap().revision, 2),
            other => panic!("harusnya konflik: {other:?}"),
        }
        let seen = server.join().unwrap();
        assert!(seen[0].starts_with("PUT /api/v1/sync/files?"), "{}", seen[0]);
        assert!(seen[0].contains("base_revision=1"), "{}", seen[0]);
        assert!(seen[0].contains("name=braket"), "{}", seen[0]);
        assert!(!seen[0].contains("force"), "{}", seen[0]);
    }

    #[test]
    fn quota_and_plain_text_413_are_classified() {
        let (url, server) = scripted_server(vec![
            (403, r#"{"success":false,"error":"Kuota habis","code":"quota_exceeded"}"#.into()),
            (413, "length limit exceeded".into()),
        ]);
        let mut s = CloudSession::new(&url, test_account());
        assert!(matches!(s.upload_file("a.ducad", b"x", 0, false), Err(SyncError::QuotaExceeded(m)) if m == "Kuota habis"));
        assert!(matches!(s.upload_file("a.ducad", b"x", 0, true), Err(SyncError::TooLarge(_))));
        server.join().unwrap();
    }

    #[test]
    fn refresh_profile_updates_tier_and_renews_token_on_401() {
        let user = r#"{"id":"u-1","email":"a@b.c","license_tier":"Pro"}"#;
        let token = format!(
            r#"{{"success":true,"data":{{"access_token":"baru","refresh_token":"segar-baru","expires_in":3600,"user":{user}}}}}"#
        );
        let (url, server) = scripted_server(vec![
            (401, r#"{"success":false,"error":"x"}"#.into()),
            (200, token),
            (200, format!(r#"{{"success":true,"data":{user}}}"#)),
        ]);
        let mut acc = test_account();
        acc.license_tier = "Free".into();
        let mut s = CloudSession::new(&url, acc);
        s.refresh_profile().unwrap();
        assert!(s.account_changed());
        let acc = s.into_account();
        assert_eq!(acc.license_tier, "Pro");
        assert_eq!(acc.refresh_token, "segar-baru");
        let seen = server.join().unwrap();
        assert!(seen[2].starts_with("GET /api/v1/users/me") && seen[2].contains("Bearer baru"));
    }

    #[test]
    fn rejected_refresh_means_session_expired() {
        let (url, server) = scripted_server(vec![
            (401, r#"{"success":false,"error":"x"}"#.into()),
            (401, r#"{"success":false,"error":"x"}"#.into()),
        ]);
        let mut s = CloudSession::new(&url, test_account());
        assert_eq!(s.get_prefs().unwrap_err(), SyncError::SessionExpired);
        server.join().unwrap();
    }

    #[test]
    fn prefs_round_trip() {
        let (url, server) = scripted_server(vec![(
            200,
            r#"{"success":true,"data":{"prefs":{"language":"En"},"revision":3,"updated_at":"2026-10-20T10:00:00Z"}}"#.into(),
        )]);
        let mut s = CloudSession::new(&url, test_account());
        let p = s.put_prefs(&serde_json::json!({"language":"En"})).unwrap();
        assert_eq!(p.revision, 3);
        assert_eq!(p.prefs.unwrap()["language"], "En");
        server.join().unwrap();
    }

    #[test]
    fn path_segment_strips_traversal() {
        assert_eq!(path_segment("../../users/me"), "usersme");
        assert_eq!(path_segment("3f2a-11"), "3f2a-11");
    }
}
