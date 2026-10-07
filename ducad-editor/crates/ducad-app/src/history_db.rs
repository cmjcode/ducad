//! Manajemen Penyimpanan SQLite untuk Riwayat Aktivitas Pengguna (Maksimal 100 data).
//! Menyimpan log aktivitas beserta snapshot dokumen (JSON) untuk fitur time-travel restore.

use std::path::PathBuf;
use chrono::Local;
use rusqlite::{params, Connection};
use ducad_ui::{ActivityItemInfo, ActivityKindUi};

pub struct HistoryDb {
    conn: Option<Connection>,
    #[allow(dead_code)]
    db_path: PathBuf,
    /// Cabang aktif tempat entri baru dicatat (default `"main"`).
    branch: String,
    /// Titik cabang untuk entri PERTAMA di cabang yang baru dibuat.
    pending_parent: Option<i64>,
}

/// Nama cabang utama.
pub const MAIN_BRANCH: &str = "main";

/// Migrasi skema (idempoten): kolom yang sudah ada membuat `ALTER` gagal dan
/// kegagalan itu memang diabaikan — pola yang sama dengan kolom `snapshot`.
fn migrate(c: &Connection) {
    let _ = c.execute("ALTER TABLE activity_log ADD COLUMN snapshot TEXT", []);
    let _ = c.execute("ALTER TABLE activity_log ADD COLUMN parent_id INTEGER", []);
    let _ = c.execute(
        "ALTER TABLE activity_log ADD COLUMN branch TEXT NOT NULL DEFAULT 'main'",
        [],
    );
}

impl Default for HistoryDb {
    fn default() -> Self {
        Self::new()
    }
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
                        details TEXT NOT NULL,
                        snapshot TEXT
                    );
                ";
                if let Err(e) = c.execute(init_sql, []) {
                    log::error!("Gagal inisialisasi tabel SQLite activity_log: {}", e);
                }
                // Migrasi kolom (snapshot, parent_id, branch) pada tabel versi lama.
                migrate(&c);
                Some(c)
            }
            Err(e) => {
                log::error!("Gagal membuka database SQLite di {:?}: {}", path, e);
                None
            }
        };

        Self {
            conn,
            db_path: path,
            branch: MAIN_BRANCH.to_string(),
            pending_parent: None,
        }
    }

    /// Cabang aktif.
    pub fn current_branch(&self) -> &str {
        &self.branch
    }

    /// Mulai cabang baru `cabang-<n>` dari entri `id`: entri berikutnya
    /// dicatat di cabang itu dengan `parent_id = id`. Merge otomatis di luar
    /// lingkup.
    pub fn start_branch_from(&mut self, id: i64) -> String {
        let existing: i64 = self
            .conn
            .as_ref()
            .and_then(|c| {
                c.query_row(
                    "SELECT COUNT(DISTINCT branch) FROM activity_log WHERE branch != ?1",
                    params![MAIN_BRANCH],
                    |r| r.get(0),
                )
                .ok()
            })
            .unwrap_or(0);
        let mut n = existing + 1;
        let taken = |name: &str| -> bool {
            self.conn
                .as_ref()
                .and_then(|c| {
                    c.query_row(
                        "SELECT COUNT(*) FROM activity_log WHERE branch = ?1",
                        params![name],
                        |r| r.get::<_, i64>(0),
                    )
                    .ok()
                })
                .unwrap_or(0)
                > 0
        };
        while taken(&format!("cabang-{n}")) {
            n += 1;
        }
        self.branch = format!("cabang-{n}");
        self.pending_parent = Some(id);
        self.branch.clone()
    }

    /// `<data_dir>/ducad_history.db` (desktop `$HOME/.ducad`, tablet di
    /// dalam sandbox aplikasi; lihat `platform::data_dir`).
    pub(crate) fn resolve_db_path() -> PathBuf {
        crate::platform::data_dir().join("ducad_history.db")
    }

    /// Catat aktivitas baru ke SQLite bersama snapshot dokumen JSON, dan batasi maksimal 100 entri terbaru.
    pub fn log_activity(
        &mut self,
        kind: ActivityKindUi,
        action: &str,
        details: &str,
        snapshot: Option<&str>,
    ) {
        let Some(conn) = &mut self.conn else { return };

        let now = Local::now();
        let timestamp = now.format("%H:%M:%S").to_string();
        let kind_str = match kind {
            ActivityKindUi::Sketch2D => "2D",
            ActivityKindUi::Solid3D => "3D",
        };

        let parent = self.pending_parent.take();
        let res = conn.execute(
            "INSERT INTO activity_log (timestamp, kind, action, details, snapshot, parent_id, branch) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![timestamp, kind_str, action, details, snapshot, parent, self.branch],
        );

        if let Err(e) = res {
            log::warn!("Gagal mencatat riwayat aktivitas ke SQLite: {}", e);
            return;
        }

        // Pangkas data agar hanya tersisa 100 entri terbaru — kecuali titik
        // cabang (entri yang dirujuk `parent_id` entri lain) dan entri
        // pembuka cabang (yang menyimpan `parent_id`): tanpa yang kedua,
        // rujukan hilang lebih dulu dan titik cabang ikut terpangkas.
        let prune_res = conn.execute(
            "DELETE FROM activity_log \
             WHERE id NOT IN (SELECT id FROM activity_log ORDER BY id DESC LIMIT 100) \
               AND parent_id IS NULL \
               AND id NOT IN (SELECT parent_id FROM activity_log WHERE parent_id IS NOT NULL)",
            [],
        );
        if let Err(e) = prune_res {
            log::warn!("Gagal memangkas riwayat aktivitas lama: {}", e);
        }
    }

    /// Ambil snapshot JSON berdasarkan `id` entri log.
    pub fn get_snapshot(&self, id: i64) -> Option<String> {
        let conn = self.conn.as_ref()?;
        let mut stmt = conn
            .prepare("SELECT snapshot FROM activity_log WHERE id = ?1")
            .ok()?;
        let snap: Option<String> = stmt.query_row(params![id], |row| row.get(0)).ok()?;
        snap
    }

    /// Muat seluruh riwayat aktivitas terbaru dari database SQLite (hingga 100 entri, urut terbaru di atas).
    pub fn load_activities(&self) -> Vec<ActivityItemInfo> {
        let Some(conn) = &self.conn else { return Vec::new() };

        let mut stmt = match conn.prepare(
            "SELECT id, timestamp, kind, action, details, branch FROM activity_log ORDER BY id DESC LIMIT 100",
        ) {
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
            let branch: String = row.get(5)?;

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
                branch,
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
        self.branch = MAIN_BRANCH.to_string();
        self.pending_parent = None;
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
                details TEXT NOT NULL,
                snapshot TEXT
            );
        ";
        conn.execute(init_sql, []).unwrap();
        migrate(&conn);
        Self {
            conn: Some(conn),
            db_path: PathBuf::from(":memory:"),
            branch: MAIN_BRANCH.to_string(),
            pending_parent: None,
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

        db.log_activity(ActivityKindUi::Sketch2D, "Line", "Panjang 50.0mm", Some("{\"mock\": 1}"));
        db.log_activity(ActivityKindUi::Solid3D, "Extrude", "Tinggi 20.0mm", Some("{\"mock\": 2}"));

        let items = db.load_activities();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].action, "Extrude");
        assert_eq!(items[0].kind, ActivityKindUi::Solid3D);
        assert_eq!(items[1].action, "Line");
        assert_eq!(items[1].kind, ActivityKindUi::Sketch2D);

        let snap1 = db.get_snapshot(items[0].id);
        assert_eq!(snap1.as_deref(), Some("{\"mock\": 2}"));
    }

    #[test]
    fn test_history_db_100_limit_pruning() {
        let mut db = HistoryDb::in_memory();

        for i in 1..=120 {
            db.log_activity(
                ActivityKindUi::Sketch2D,
                &format!("Aksi #{}", i),
                &format!("Detail #{}", i),
                None,
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
    fn branch_from_history_records_parent_and_survives_pruning() {
        let mut db = HistoryDb::in_memory();
        db.log_activity(ActivityKindUi::Solid3D, "A", "", Some("{}"));
        db.log_activity(ActivityKindUi::Solid3D, "B", "", Some("{}"));
        let a_id = db.load_activities()[1].id;
        let name = db.start_branch_from(a_id);
        assert_eq!(name, "cabang-1");
        db.log_activity(ActivityKindUi::Solid3D, "C", "", None);
        db.log_activity(ActivityKindUi::Solid3D, "D", "", None);
        let items = db.load_activities();
        assert_eq!(items[0].branch, "cabang-1");
        assert_eq!(items[2].branch, "main");
        let conn = db.conn.as_ref().unwrap();
        let parent: Option<i64> = conn
            .query_row("SELECT parent_id FROM activity_log WHERE action = 'C'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(parent, Some(a_id), "entri pertama cabang menunjuk titik cabang");
        let parent_d: Option<i64> = conn
            .query_row("SELECT parent_id FROM activity_log WHERE action = 'D'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(parent_d, None);

        // 150 entri baru: titik cabang "A" tetap ada meski di luar 100 terbaru.
        for i in 0..150 {
            db.log_activity(ActivityKindUi::Sketch2D, &format!("x{i}"), "", None);
        }
        assert!(db.get_snapshot(a_id).is_some(), "titik cabang tidak ikut dipangkas");
        assert_eq!(db.start_branch_from(a_id), "cabang-2");
    }

    #[test]
    fn migration_keeps_legacy_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE activity_log (id INTEGER PRIMARY KEY AUTOINCREMENT, timestamp TEXT NOT NULL, \
             kind TEXT NOT NULL, action TEXT NOT NULL, details TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO activity_log (timestamp, kind, action, details) VALUES ('10:00:00', '3D', 'Lama', 'd')",
            [],
        )
        .unwrap();
        migrate(&conn);
        migrate(&conn);
        let db = HistoryDb {
            conn: Some(conn),
            db_path: PathBuf::from(":memory:"),
            branch: MAIN_BRANCH.to_string(),
            pending_parent: None,
        };
        let items = db.load_activities();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].action, "Lama");
        assert_eq!(items[0].branch, "main");
    }

    #[test]
    fn test_history_db_clear() {
        let mut db = HistoryDb::in_memory();
        db.log_activity(ActivityKindUi::Sketch2D, "Circle", "Radius 15.0mm", None);
        assert_eq!(db.load_activities().len(), 1);

        db.clear();
        assert_eq!(db.load_activities().len(), 0);
    }
}

#[cfg(test)]
mod app_branch_tests {
    use crate::app::DuCADApp;
    use ducad_ui::ActivityKindUi;

    #[test]
    fn branch_from_history_restores_and_switches_branch() {
        let mut app = DuCADApp::new_for_test();
        let geo = ducad_engine::compute::primitive(
            &ducad_engine::compute::PrimitiveShape::Box { size: [10.0, 10.0, 10.0], centered: false },
            [0.0; 3],
        )
        .unwrap();
        let id = app.model.doc.add_body("A");
        app.model.geometry.insert(id, geo);
        app.record_activity(ActivityKindUi::Solid3D, "Satu body", "");
        let first = app.activity_cache[0].id;
        let geo2 = ducad_engine::compute::primitive(
            &ducad_engine::compute::PrimitiveShape::Sphere { r: 2.0 },
            [30.0, 0.0, 0.0],
        )
        .unwrap();
        let id2 = app.model.doc.add_body("B");
        app.model.geometry.insert(id2, geo2);
        app.record_activity(ActivityKindUi::Solid3D, "Dua body", "");
        assert_eq!(app.model.doc.bodies.len(), 2);

        app.branch_from_history(first);
        assert_eq!(app.model.doc.bodies.len(), 1, "snapshot entri pertama dipulihkan");
        assert_eq!(app.history_db.current_branch(), "cabang-1");
        assert_eq!(app.activity_cache[0].branch, "cabang-1");
        assert_eq!(app.activity_cache[1].branch, "main");
    }
}
