//! I/O DUCAD (Fase 5): format native `.ducad` (JSON versi-ed, lihat
//! `native`), STEP via `ducad-kernel` (`step_io`), DXF minimal R12 untuk
//! interop AutoCAD (`dxf`), export STL biner/OBJ (`mesh_export`).
//!
//! Prinsip yang sama di semua modul: crate ini TIDAK punya struct
//! serializable duplikat dari `ducad-sketch`/`ducad-kernel` — `Sketch`/
//! `Entity`/`Constraint` di-derive `Serialize`/`Deserialize` langsung di
//! `ducad-sketch` (lihat catatan di sana soal `slotmap` serde), dan
//! geometri B-rep lewat `KernelShape::to_step_string`/`from_step_string`
//! di `ducad-kernel`. Crate ini murni orkestrasi format file di atasnya.

// Prinsip "jangan pernah crash" (ADR 0009): tidak ada unwrap/expect/panic di
// kode produksi; modul tes dikecualikan.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
pub mod drawing;
pub mod dxf;
pub mod flat_dxf;
pub mod glb;
pub mod mesh_export;
pub mod mesh_formats;
pub mod external;
pub mod native;
pub mod pdf;
pub mod pdf_import;
pub mod step_io;
pub mod svg;
pub mod svg_import;

pub use drawing::{
    BomItem, BomTable, CalloutBalloon, DimensionAnnotation, DrawingSheet, PaperSize,
    SheetViewPlacement, TextAnnotation, TitleBlockInfo,
};
pub use dxf::export_drawing_sheet;
pub use glb::{export_glb_bytes, write_glb, write_glb_with_options, GlbExportOptions};
pub use mesh_export::{read_stl, read_stl_ascii_str, read_stl_binary_bytes, write_obj, write_stl_binary};
pub use pdf::export_pdf;
pub use svg::{
    export_drawing_sheet_svg, export_sketch_svg, export_sketch_svg_string,
    export_sketch_svg_with_options, export_vector_snapshot_svg,
    export_vector_snapshot_svg_layers, export_vector_snapshot_svg_string,
    export_vector_snapshot_svg_with_options, SvgSketchOptions,
    SvgSnapshotOptions,
};
pub use native::needs_v3;
pub use svg_import::{import_svg_entities, SvgImport, SvgImportOptions};

/// Lock test SATU-SATUNYA untuk seluruh binary test crate ini — dipakai
/// tes yang membangun B-rep sungguhan (`native`, `step_io`, `external`,
/// `svg`) supaya tes berat (boolean, STEP) jalan serial dan waktunya
/// stabil di runner CI.
#[cfg(test)]
pub(crate) mod test_lock {
    pub static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
