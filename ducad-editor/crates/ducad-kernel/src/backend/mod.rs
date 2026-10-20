//! Backend kernel geometri: SolidCraft kernel di atas truck (`truck/`).
//!
//! API publik crate ini (modul `csg`, `modify`, `shape`, …) menyimpan tipe &
//! logika netral, lalu me-re-export fungsi khusus backend dari
//! `crate::backend::<modul>`. Lapisan ini dipertahankan supaya penggantian
//! kernel berikutnya tetap terisolasi di sini.

mod truck;
pub use truck::*;
