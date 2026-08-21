//! Manajemen Penyimpanan SQLite untuk Riwayat Aktivitas Pengguna (Maksimal 100 data).

use std::path::PathBuf;
use chrono::Local;
use rusqlite::{params, Connection};
use ducad_ui::{ActivityItemInfo, ActivityKindUi};

pub struct HistoryDb {
    conn: Option<Connection>,
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl HistoryDb {
    pub fn new() -> Self {
        let path = Self::resolve_db_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = match Connection::open(&path) {
            Ok(c) => {
                let init_sql = "
                    CREATE TABLE IF NOT EXISTS activity_log (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        timestamp TEXT NOT NULL,
                        kind TEXT NOT NULL,
                        action TEXT NOT NULL,
                        details TEXT NOT NULL
                    );
                ";
                if let Err(e) = c.execute(init_sql, []) {
                    log::error!("Gagal inisialisasi tabel SQLite activity_log: {}", e);
                }
                Some(c)
            }
            Err(e) => {
                log::error!("Gagal membuka database SQLite di {:?}: {}", path, e);
                None
            }
        };

        Self { conn, db_path: path }
    }

    #[cfg(target_os = "ios")]
    fn resolve_db_path() -> PathBuf {
        crate::file_io::ios_documents_dir().join("ducad_history.db")
    }

    #[cfg(not(target_os = "ios"))]
    fn resolve_db_path() -> PathBuf {
        if let Some(home) = std::env::var_os("HOME") {
            let p = PathBuf::from(home).join(".ducad").join("ducad_history.db");
            return p;
        }
        PathBuf::from("ducad_history.db")
    }

    /// Catat aktivitas baru ke SQLite dan batasi maksimal 100 entri terbaru.
    pub fn log_activity(&mut self, kind: ActivityKindUi, action: &str, details: &str) {
        let Some(conn) = &mut self.conn else { return };

        let now = Local::now();
        let timestamp = now.format("%H:%M:%S").to_string();
        let kind_str = match kind {
            ActivityKindUi::Sketch2D => "2D",
            ActivityKindUi::Solid3D => "3D",
        };

        let res = conn.execute(
            "INSERT INTO activity_log (timestamp, kind, action, details) VALUES (?1, ?2, ?3, ?4)",
            params![timestamp, kind_str, action, details],
        );

        if let Err(e) = res {
            log::warn!("Gagal mencatat riwayat aktivitas ke SQLite: {}", e);
            return;
        }

        // Pangkas data agar hanya tersisa 100 entri terbaru
        let prune_res = conn.execute(
            "DELETE FROM activity_log WHERE id NOT IN (SELECT id FROM activity_log ORDER BY id DESC LIMIT 100)",
            [],
        );
        if let Err(e) = prune_res {
            log::warn!("Gagal memangkas riwayat aktivitas lama: {}", e);
        }
    }

    /// Muat seluruh riwayat aktivitas terbaru dari database SQLite (hingga 100 entri, urut terbaru di atas).
    pub fn load_activities(&self) -> Vec<ActivityItemInfo> {
        let Some(conn) = &self.conn else { return Vec::new() };

        let mut stmt = match conn.prepare("SELECT id, timestamp, kind, action, details FROM activity_log ORDER BY id DESC LIMIT 100") {
            Ok(s) => s,
            Err(e) => {
                log::error!("Gagal menyiapkan query riwayat aktivitas: {}", e);
                return Vec::new();
            }
        };

        let rows = match stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let timestamp: String = row.get(1)?;
            let kind_str: String = row.get(2)?;
            let action: String = row.get(3)?;
            let details: String = row.get(4)?;

            let kind = if kind_str == "2D" {
                ActivityKindUi::Sketch2D
            } else {
                ActivityKindUi::Solid3D
            };

            Ok(ActivityItemInfo {
                id,
                timestamp,
                kind,
                action,
                details,
            })
        }) {
            Ok(r) => r,
            Err(e) => {
                log::error!("Gagal membaca baris riwayat aktivitas: {}", e);
                return Vec::new();
            }
        };

        rows.filter_map(Result::ok).collect()
    }

    /// Hapus semua riwayat aktivitas dari database SQLite.
    pub fn clear(&mut self) {
        if let Some(conn) = &mut self.conn {
            let _ = conn.execute("DELETE FROM activity_log", []);
        }
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        let init_sql = "
            CREATE TABLE IF NOT EXISTS activity_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                kind TEXT NOT NULL,
                action TEXT NOT NULL,
                details TEXT NOT NULL
            );
        ";
        conn.execute(init_sql, []).unwrap();
        Self {
            conn: Some(conn),
            db_path: PathBuf::from(":memory:"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_db_insert_and_load() {
        let mut db = HistoryDb::in_memory();
        assert!(db.load_activities().is_empty());

        db.log_activity(ActivityKindUi::Sketch2D, "Line", "Panjang 50.0mm");
        db.log_activity(ActivityKindUi::Solid3D, "Extrude", "Tinggi 20.0mm");

        let items = db.load_activities();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].action, "Extrude");
        assert_eq!(items[0].kind, ActivityKindUi::Solid3D);
        assert_eq!(items[1].action, "Line");
        assert_eq!(items[1].kind, ActivityKindUi::Sketch2D);
    }

    #[test]
    fn test_history_db_100_limit_pruning() {
        let mut db = HistoryDb::in_memory();

        for i in 1..=120 {
            db.log_activity(
                ActivityKindUi::Sketch2D,
                &format!("Aksi #{}", i),
                &format!("Detail #{}", i),
            );
        }

        let items = db.load_activities();
        assert_eq!(items.len(), 100);
        // Item paling baru harus Aksi #120
        assert_eq!(items[0].action, "Aksi #120");
        // Item paling lama di 100 list harus Aksi #21 (karena 1..=20 sudah dipangkas)
        assert_eq!(items[99].action, "Aksi #21");
    }

    #[test]
    fn test_history_db_clear() {
        let mut db = HistoryDb::in_memory();
        db.log_activity(ActivityKindUi::Sketch2D, "Circle", "Radius 15.0mm");
        assert_eq!(db.load_activities().len(), 1);

        db.clear();
        assert_eq!(db.load_activities().len(), 0);
    }
}

