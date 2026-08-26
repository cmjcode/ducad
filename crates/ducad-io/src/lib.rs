//! I/O DUCAD (Fase 5): format native `.ducad` (JSON versi-ed, lihat
//! `native`), STEP via kernel OCCT (`step_io`), DXF minimal R12 untuk
//! interop AutoCAD (`dxf`), export STL biner/OBJ (`mesh_export`).
//!
//! Prinsip yang sama di semua modul: crate ini TIDAK punya struct
//! serializable duplikat dari `ducad-sketch`/`ducad-kernel` — `Sketch`/
//! `Entity`/`Constraint` di-derive `Serialize`/`Deserialize` langsung di
//! `ducad-sketch` (lihat catatan di sana soal `slotmap` serde), dan
//! geometri B-rep lewat `KernelShape::to_step_string`/`from_step_string`
//! di `ducad-kernel`. Crate ini murni orkestrasi format file di atasnya.

pub mod drawing;
pub mod dxf;
pub mod glb;
pub mod mesh_export;
pub mod native;
pub mod pdf;
pub mod step_io;
pub mod svg;

pub use drawing::{
    DimensionAnnotation, DrawingSheet, PaperSize, SheetViewPlacement, TextAnnotation, TitleBlockInfo,
};
pub use dxf::export_drawing_sheet;
pub use glb::{export_glb_bytes, write_glb, write_glb_with_options, GlbExportOptions};
pub use pdf::export_pdf;
pub use svg::{
    export_drawing_sheet_svg, export_sketch_svg, export_sketch_svg_string,
    export_sketch_svg_with_options, SvgSketchOptions,
};

/// Lock test SATU-SATUNYA untuk seluruh binary test crate ini — dipakai
/// `native`/`step_io`, dua modul yang sama-sama menyentuh jalur transfer
/// STEP OCCT (lewat `ducad_kernel::KernelShape`). Sama alasan dengan
/// `ducad-kernel::tests::TEST_LOCK`: jalur itu TIDAK thread-safe, dan
/// `cargo test` menjalankan semua test SATU binary (semua modul di crate
/// ini) di banyak thread sekaligus — lock per-modul sendiri-sendiri TIDAK
/// cukup (ditemukan lewat test yang gagal acak: pola bug yang sama persis
/// ditemukan lagi, kali ini lintas modul bukan lintas file).
#[cfg(test)]
pub(crate) mod occt_test_lock {
    pub static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
