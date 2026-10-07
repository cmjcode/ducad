//! Memori MNEMONIC yang tertaut langsung (P11.5).
//!
//! Di macOS, agent eksternal memakai MNEMONIC sebagai server MCP terpisah
//! (`.mcp.json`). iPadOS tidak bisa menjalankan proses anak, jadi di sana
//! satu-satunya jalan adalah memanggil `VaultService` di dalam proses.
//! Pustaka MNEMONIC dibangun dengan fitur `gui` MATI (lihat `Cargo.toml`
//! repo itu) supaya eframe/egui/pdfium/rfd tidak ikut tertarik.
//!
//! Pembagian memori mengikuti `AGENTS.md` vault: geometri dan oplog TIDAK
//! pernah masuk ke sini — hanya preferensi, standar, pelajaran, keputusan
//! proyek, dan log sesi.
//!
//! `VaultService` memegang koneksi SQLite dan butuh `&mut` untuk hampir
//! semua operasinya, jadi ia hidup di balik `Mutex`: asisten AI (P11.4)
//! memanggilnya dari thread latar, UI thread dari command palette.

use std::path::PathBuf;
use std::sync::Mutex;

use mnemonic::api::memory::types::{RecallRequest, RememberRequest};
use mnemonic::api::VaultService;

/// Folder pelajaran dan log sesi di vault (lihat `AGENTS.md`).
const LESSONS_FOLDER: &str = "Lessons";
const SESSIONS_FOLDER: &str = "Sessions";
/// Tag wajib untuk setiap catatan yang ditulis DUCAD.
const TAG: &str = "ducad";
/// Nama harness yang dicatat di setiap tulisan.
const AGENT: &str = "ducad-app";
/// Anggaran recall sebelum memodelkan (AGENTS.md §"Sebelum memodelkan").
const RECALL_BUDGET_TOKENS: usize = 1500;

/// Vault memori yang bisa dipakai lintas thread.
pub struct VaultMemory {
    inner: Mutex<VaultService>,
    root: PathBuf,
}

/// Lokasi vault bawaan: `$HOME/DUCAD-Memory` di desktop, folder Documents
/// aplikasi di iPadOS (satu-satunya tempat yang bisa ditulis).
pub fn default_root() -> PathBuf {
    if crate::platform::is_mobile() {
        crate::platform::documents_dir().join("DUCAD-Memory")
    } else {
        match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join("DUCAD-Memory"),
            None => PathBuf::from("DUCAD-Memory"),
        }
    }
}

impl VaultMemory {
    /// Buka (atau buat) vault di `root`.
    pub fn open(root: PathBuf) -> anyhow::Result<Self> {
        let service = VaultService::open(root.clone())?;
        Ok(Self {
            inner: Mutex::new(service),
            root,
        })
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Ingat konteks yang relevan sebelum memodelkan. Hasilnya berupa
    /// potongan teks siap dipakai sebagai `lessons` untuk asisten (P11.2).
    ///
    /// `semantic: false` — pencarian vektor mengunduh model ±120 MB saat
    /// pertama dipakai; pencarian leksikal sudah cukup untuk vault CAD dan
    /// selalu tersedia offline (lihat P4.1).
    pub fn recall(&self, query: &str, max_items: usize) -> Vec<String> {
        let Ok(mut svc) = self.inner.lock() else {
            return Vec::new();
        };
        let req = RecallRequest {
            query: query.to_string(),
            budget_tokens: RECALL_BUDGET_TOKENS,
            semantic: false,
            ..RecallRequest::default()
        };
        match svc.recall(&req) {
            Ok(r) => r
                .items
                .into_iter()
                .take(max_items)
                .map(|i| match i.section {
                    Some(s) => format!("{} › {}: {}", i.title, s, i.text),
                    None => format!("{}: {}", i.title, i.text),
                })
                .collect(),
            Err(e) => {
                log::warn!("memori: recall gagal: {e:#}");
                Vec::new()
            }
        }
    }

    /// Catat satu pelajaran yang SUDAH terbukti (checks hijau) ke
    /// `Lessons/`. Duplikat ditolak oleh MNEMONIC sendiri.
    pub fn remember_lesson(&self, title: &str, text: &str) -> anyhow::Result<String> {
        self.write(
            text,
            Some(title.to_string()),
            LESSONS_FOLDER,
            vec![TAG.to_string(), "lesson".to_string()],
            None,
        )
    }

    /// Tambahkan satu baris ke log sesi `Sessions/YYYY-MM-DD.md`.
    ///
    /// `remember` dengan `ref` hanya bisa menambah ke catatan yang SUDAH
    /// ada, jadi hari pertama sebuah sesi catatannya dibuat dulu.
    pub fn append_session(&self, text: &str) -> anyhow::Result<String> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let tags = vec![TAG.to_string(), "session".to_string()];
        let note = format!("{SESSIONS_FOLDER}/{today}.md");
        match self.write(
            text,
            Some(today.clone()),
            SESSIONS_FOLDER,
            tags.clone(),
            Some(note),
        ) {
            Ok(status) => Ok(status),
            Err(_) => self.write(text, Some(today), SESSIONS_FOLDER, tags, None),
        }
    }

    /// Catat keputusan desain satu part ke `Projects/<part>.md`.
    pub fn remember_project(&self, part: &str, text: &str) -> anyhow::Result<String> {
        self.write(
            text,
            Some(part.to_string()),
            "Projects",
            vec![TAG.to_string(), "project".to_string()],
            Some(format!("Projects/{part}.md")),
        )
    }

    fn write(
        &self,
        text: &str,
        title: Option<String>,
        folder: &str,
        tags: Vec<String>,
        into: Option<String>,
    ) -> anyhow::Result<String> {
        let mut svc = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("kunci vault memori rusak"))?;
        let req = RememberRequest {
            text: text.to_string(),
            title,
            folder: Some(folder.to_string()),
            tags,
            r#ref: into,
            agent: Some(AGENT.to_string()),
            // Sama seperti `recall`: tanpa model embedding.
            semantic: false,
            ..RememberRequest::default()
        };
        let out = svc.remember(&req)?;
        Ok(out.status.to_string())
    }

    /// Ringkasan vault untuk chip/status.
    pub fn summary(&self) -> Option<String> {
        let svc = self.inner.lock().ok()?;
        let o = svc.vault_overview();
        Some(format!("{} catatan", o.notes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_vault() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ducad-memory-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("buat folder vault");
        dir
    }

    #[test]
    fn remember_then_recall_roundtrip() {
        let root = temp_vault();
        let mem = VaultMemory::open(root.clone()).expect("buka vault");
        let status = mem
            .remember_lesson(
                "Fillet melebihi tepi terpendek",
                "Gejala: fillet R3 gagal pada tepi 2,1 mm. Perbaikan: radius 1,8 mm.",
            )
            .expect("tulis pelajaran");
        assert_eq!(status, "created");

        let hits = mem.recall("fillet tepi pendek", 5);
        assert!(
            hits.iter().any(|h| h.contains("1,8")),
            "pelajaran harus bisa diingat kembali: {hits:?}"
        );
        assert!(mem.summary().is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn session_note_is_named_by_date() {
        let root = temp_vault();
        let mem = VaultMemory::open(root.clone()).expect("buka vault");
        mem.append_session("Membuat bracket L 50x30x5.")
            .expect("tulis log sesi");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let path = root.join(SESSIONS_FOLDER).join(format!("{today}.md"));
        assert!(path.exists(), "{} harus ada", path.display());
        let _ = std::fs::remove_dir_all(&root);
    }
}
