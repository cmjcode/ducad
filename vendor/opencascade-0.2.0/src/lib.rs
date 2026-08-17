use thiserror::Error;

pub mod adhoc;
pub mod angle;
pub mod mesh;
pub mod primitives;
pub mod workplane;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to write STL file")]
    StlWriteFailed,
    #[error("Failed to read STEP file")]
    StepReadFailed,
    #[error("Failed to write STEP file")]
    StepWriteFailed,
    // PATCH (CADRAW, lihat vendor/README.md — CADRAW Fase 3, offset shell
    // per-face): variant baru, dipakai `Shape::offset_on_face` utk
    // membungkus kegagalan `BRepOffset_MakeOffset` (Initialize/
    // SetOffsetOnFace/MakeOffsetShape/Shape masing-masing bisa gagal
    // secara geometris di OCCT — sudah diterjemahkan jadi `Result<>` cxx
    // rapi oleh `opencascade-sys`, tinggal disalurkan ke sini).
    #[error("BRepOffset_MakeOffset gagal: {0}")]
    OffsetOnFaceFailed(String),
}
