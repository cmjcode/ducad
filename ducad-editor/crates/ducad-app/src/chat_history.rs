//! Riwayat chat AI (P13.3): tabel `chat_sessions` di `ducad_history.db`
//! (berkas yang sama dengan riwayat aktivitas). Lokal saja, tidak pernah
//! disinkronkan; maksimal [`MAX_SESSIONS`] sesi terbaru. Gambar hasil tool
//! tidak disimpan.

use chrono::Local;
use ducad_chat::ConvMessage;
use rusqlite::{params, Connection};

pub const MAX_SESSIONS: i64 = 200;

pub struct ChatHistory {
    conn: Option<Connection>,
}

const INIT: &str = "CREATE TABLE IF NOT EXISTS chat_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    updated TEXT NOT NULL,
    messages TEXT NOT NULL
)";

impl ChatHistory {
    pub fn open_default() -> Self {
        let path = crate::history_db::HistoryDb::resolve_db_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(&path)
            .map_err(|e| log::warn!("riwayat chat: gagal membuka {}: {e}", path.display()))
            .ok();
        Self::init(conn)
    }

    pub fn in_memory() -> Self {
        Self::init(Connection::open_in_memory().ok())
    }

    fn init(conn: Option<Connection>) -> Self {
        let conn = conn.and_then(|c| match c.execute(INIT, []) {
            Ok(_) => Some(c),
            Err(e) => {
                log::warn!("riwayat chat: gagal membuat tabel: {e}");
                None
            }
        });
        Self { conn }
    }

    /// Simpan (baru bila `id` kosong). Mengembalikan id sesi.
    pub fn save(&self, id: Option<i64>, title: &str, conv: &[ConvMessage]) -> Option<i64> {
        let c = self.conn.as_ref()?;
        let json = serde_json::to_string(conv).ok()?;
        let now = Local::now().format("%Y-%m-%d %H:%M").to_string();
        let id = match id {
            Some(id) => {
                c.execute(
                    "UPDATE chat_sessions SET updated = ?1, messages = ?2 WHERE id = ?3",
                    params![now, json, id],
                )
                .ok()?;
                id
            }
            None => {
                c.execute(
                    "INSERT INTO chat_sessions (title, updated, messages) VALUES (?1, ?2, ?3)",
                    params![title, now, json],
                )
                .ok()?;
                c.last_insert_rowid()
            }
        };
        let _ = c.execute(
            "DELETE FROM chat_sessions WHERE id NOT IN (SELECT id FROM chat_sessions ORDER BY id DESC LIMIT ?1)",
            params![MAX_SESSIONS],
        );
        Some(id)
    }

    /// `(id, judul, waktu)` terbaru dulu.
    pub fn list(&self) -> Vec<(i64, String, String)> {
        let Some(c) = self.conn.as_ref() else {
            return Vec::new();
        };
        let Ok(mut st) = c.prepare("SELECT id, title, updated FROM chat_sessions ORDER BY id DESC")
        else {
            return Vec::new();
        };
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn load(&self, id: i64) -> Option<Vec<ConvMessage>> {
        let c = self.conn.as_ref()?;
        let json: String = c
            .query_row(
                "SELECT messages FROM chat_sessions WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .ok()?;
        serde_json::from_str(&json)
            .map_err(|e| log::warn!("riwayat chat {id} rusak: {e}"))
            .ok()
    }

    pub fn delete(&self, id: i64) {
        if let Some(c) = self.conn.as_ref() {
            let _ = c.execute("DELETE FROM chat_sessions WHERE id = ?1", params![id]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_list_load_delete() {
        let h = ChatHistory::in_memory();
        let conv = vec![ConvMessage::User { text: "hai".into() }];
        let id = h.save(None, "Sesi 1", &conv).unwrap();
        let mut longer = conv.clone();
        longer.push(ConvMessage::Assistant {
            text: "halo".into(),
            calls: vec![],
            raw: None,
        });
        assert_eq!(h.save(Some(id), "abaikan", &longer), Some(id));
        let list = h.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].1, "Sesi 1");
        assert_eq!(h.load(id).unwrap(), longer);
        h.delete(id);
        assert!(h.list().is_empty());
        assert!(h.load(id).is_none());
    }
}
