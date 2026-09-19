//! Ringkasan keadaan sesi. Versi sementara P1 (`bodies`, `warnings`);
//! diperluas menjadi laporan lengkap di P2.2.

use serde::Serialize;

use crate::model::ModelDoc;
use crate::session::{Session, SessionMeta};

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// Nama body, terurut.
    pub bodies: Vec<String>,
    /// Peringatan tingkat sesi, mis. `"oplog_stale"`.
    pub warnings: Vec<String>,
}

pub(crate) fn summarize_state(model: &ModelDoc, meta: &SessionMeta) -> Summary {
    let mut bodies: Vec<String> = model.doc.bodies.values().map(|b| b.name.clone()).collect();
    bodies.sort();
    Summary {
        bodies,
        warnings: meta.warnings.clone(),
    }
}

pub fn summarize(s: &Session) -> Summary {
    summarize_state(s.model(), s.meta())
}
