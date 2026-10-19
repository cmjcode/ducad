//! Backend kernel geometri: SolidCraft kernel di atas truck (`truck/`).
//!
//! API publik crate ini (modul `csg`, `modify`, `shape`, …) menyimpan tipe &
//! logika netral, lalu me-re-export fungsi khusus backend dari
//! `crate::backend::<modul>`. Lapisan ini dipertahankan setelah backend
//! OpenCASCADE dihapus (PLAN_MIGRASI_TRUCK F6.4) supaya penggantian kernel
//! berikutnya tetap terisolasi di sini.

mod truck;
pub use truck::*;
