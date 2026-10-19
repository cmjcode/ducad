//! Pemilihan backend kernel geometri (PLAN_MIGRASI_TRUCK §3).
//!
//! API publik crate ini (modul `csg`, `modify`, `shape`, …) TIDAK berubah
//! antar-backend. Tiap modul publik menyimpan tipe & logika netral-backend,
//! lalu me-re-export fungsi khusus backend dari `crate::backend::<modul>`:
//!
//! - `truck/` — SolidCraft kernel di atas truck (fitur `kernel-truck`, default sejak F6).
//! - `occt/`  — OpenCASCADE lewat `opencascade-rs` (fitur `kernel-occt`, opt-in
//!   selama soak 0.6.0-beta; dihapus di F6.4).
//!
//! Bila kedua fitur aktif, **OCCT yang dipakai** (fitur default truck selalu
//! ikut aktif, jadi permintaan eksplisit `kernel-occt` harus menang). Aturan
//! itu ada di `build.rs` → `cfg(backend_occt)` / `cfg(backend_truck)`.
//! `src/lib.rs` menolak build tanpa backend.

#[cfg(backend_truck)]
mod truck;
#[cfg(backend_truck)]
pub use truck::*;

#[cfg(backend_occt)]
mod occt;
#[cfg(backend_occt)]
pub use occt::*;
