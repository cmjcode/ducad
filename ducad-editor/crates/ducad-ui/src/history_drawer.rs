//! Tipe aktivitas kronologis 2D & 3D yang ditampilkan di tab Riwayat pada
//! `FeatureTreeDrawer`.
//!
//! Komponen `HistoryDrawer` lama yang berdiri sendiri sudah dipensiunkan dan
//! disatukan ke dalam `FeatureTreeDrawer`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKindUi {
    Sketch2D,
    Solid3D,
}

#[derive(Debug, Clone)]
pub struct ActivityItemInfo {
    pub id: i64,
    pub timestamp: String,
    pub kind: ActivityKindUi,
    pub action: String,
    pub details: String,
    /// Cabang histori (`"main"` atau `"cabang-<n>"`).
    pub branch: String,
}
