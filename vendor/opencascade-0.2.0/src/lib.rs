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
    // PATCH (DUCAD, lihat vendor/README.md — DUCAD Fase 3, offset shell
    // per-face): variant baru, dipakai `Shape::offset_on_face` utk
    // membungkus kegagalan `BRepOffset_MakeOffset` (Initialize/
    // SetOffsetOnFace/MakeOffsetShape/Shape masing-masing bisa gagal
    // secara geometris di OCCT — sudah diterjemahkan jadi `Result<>` cxx
    // rapi oleh `opencascade-sys`, tinggal disalurkan ke sini).
    #[error("BRepOffset_MakeOffset gagal: {0}")]
    OffsetOnFaceFailed(String),
    // PATCH (DUCAD, lihat vendor/README.md): variant baru, dipakai
    // `Shape::fillet_edge`/`fillet_edges`/`chamfer_edge`/`chamfer_edges`
    // utk membungkus kegagalan `BRepFilletAPI_MakeFillet`/
    // `BRepFilletAPI_MakeChamfer::Shape()` (`StdFail_NotDone` — radius/jarak
    // > yang bisa ditampung tepi/sudut terpilih, mis. drag gizmo rounding
    // sampai batas ujung objek) — sudah diterjemahkan jadi `Result<>` cxx
    // rapi lewat wrapper try/catch di `opencascade-sys`, tinggal disalurkan
    // ke sini (pola sama dgn `OffsetOnFaceFailed`).
    #[error("BRepFilletAPI_MakeFillet/MakeChamfer gagal: {0}")]
    FilletFailed(String),
    // PATCH (DUCAD, lihat vendor/README.md): variant baru, dipakai
    // `Shape::union`/`subtract` & `AdHocShape::union`/`subtract`/
    // `intersect` utk membungkus kegagalan `BRepAlgoAPI_Fuse`/`Cut`/
    // `Common` (`StdFail_NotDone` — kasus nyata: extrude jalur datar
    // `ducad-kernel::extrude_face` fuse/cut prism baru ke shape yang
    // tepi/sudut tetangganya sudah di-rounding, prism bertemu blend
    // surface fillet secara tangen sehingga klasifikasi boolean OCCT
    // gagal) — sudah diterjemahkan jadi `Result<>` cxx rapi lewat wrapper
    // try/catch di `opencascade-sys`, tinggal disalurkan ke sini (pola
    // sama dgn `FilletFailed`).
    #[error("BRepAlgoAPI_Fuse/Cut/Common gagal: {0}")]
    BooleanOpFailed(String),
    // PATCH (DUCAD): variant baru untuk membungkus kegagalan BRepPrimAPI_MakeRevol
    // (sumbu memotong interior profil, profil tidak tertutup, atau geometri tidak valid).
    #[error("BRepPrimAPI_MakeRevol gagal: {0}")]
    RevolveFailed(String),
    // PATCH (DUCAD): variant baru untuk membungkus kegagalan BRepOffsetAPI_MakeThickSolid
    // (mis. shape sudah berongga/shell, atau tebal dinding melebihi batas geometri).
    #[error("BRepOffsetAPI_MakeThickSolid gagal: {0}")]
    HollowFailed(String),
    // PATCH (DUCAD): variant baru untuk membungkus kegagalan BRepOffsetAPI_MakePipe
    // (mis. profil atau jalur kurva bermasalah).
    #[error("BRepOffsetAPI_MakePipe gagal: {0}")]
    PipeFailed(String),
    // PATCH (DUCAD Fase 2.1 — manufaktur plastik): variant baru untuk membungkus
    // kegagalan BRepOffsetAPI_DraftAngle (face bukan planar, sudut di luar batas
    // geometri yang bisa ditampung, neutral plane tidak valid, atau shape
    // tidak kompatibel). Sudah diterjemahkan jadi `Result<>` cxx rapi lewat
    // wrapper try/catch di `opencascade-sys`, tinggal disalurkan ke sini
    // (pola sama dgn `FilletFailed`/`BooleanOpFailed`).
    #[error("BRepOffsetAPI_DraftAngle gagal: {0}")]
    DraftAngleFailed(String),
}


