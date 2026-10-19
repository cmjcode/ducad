//! Pemilihan backend kernel geometri (PLAN_MIGRASI_TRUCK §3).
//!
//! API publik crate ini (modul `csg`, `modify`, `shape`, …) TIDAK berubah
//! antar-backend. Tiap modul publik menyimpan tipe & logika netral-backend,
//! lalu me-re-export fungsi khusus backend dari `crate::backend::<modul>`:
//!
//! - `occt/`  — OpenCASCADE lewat `opencascade-rs` (fitur `kernel-occt`, default).
//! - `truck/` — SolidCraft kernel di atas truck (fitur `kernel-truck`).
//!
//! Bila kedua fitur aktif, **truck yang dipakai**: perintah transisi seperti
//! `cargo test --workspace --features kernel-truck` (fitur default ikut aktif)
//! harus menguji backend truck. `src/lib.rs` menolak build tanpa backend.

#[cfg(feature = "kernel-truck")]
mod truck;
#[cfg(feature = "kernel-truck")]
pub use truck::*;

#[cfg(all(feature = "kernel-occt", not(feature = "kernel-truck")))]
mod occt;
#[cfg(all(feature = "kernel-occt", not(feature = "kernel-truck")))]
pub use occt::*;
