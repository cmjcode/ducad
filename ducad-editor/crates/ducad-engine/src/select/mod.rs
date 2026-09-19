//! Selector semantik face/tepi (P1.4): agent memilih geometri lewat
//! properti (`>Z`, `|Z`, `of(>Z)`, `all[kind=cylinder][r=2.75]`), engine
//! menerjemahkannya ke indeks `ducad_kernel::enumerate_*`.

pub mod eval;
pub mod parse;

/// Ringkasan tata bahasa & semantik selector untuk agent (≤ 40 baris).
pub const SELECTOR_CHEATSHEET: &str = "\
Selector face/tepi (tidak peka huruf besar; LIN_TOL 1e-4 mm, ANG_TOL 0.5 derajat)
selector := term (or|and|except term)*        -- asosiatif kiri, prioritas sama
term     := base filter*
BASE                 FACE                               TEPI
all                  semua face                         semua tepi
>A / <A              posisi centroid maks/min di A      posisi mid maks/min di A
+A / -A              normal keluar searah +A/-A         (tidak berlaku)
|A                   normal sejajar A                   garis sejajar A
#A                   normal tegak lurus A               garis tegak lurus A
largest / smallest   luas maks/min                      (tidak berlaku)
longest / shortest   (tidak berlaku)                    panjang maks/min
idx:i,j              indeks enumerasi                   indeks enumerasi
of(S)                (tidak berlaku)                    tepi dari face hasil S
(S)                  pengelompokan                      pengelompokan
A = X | Y | Z
FILTER [KEY CMP VALUE], CMP = = < > <= >=, '=' numerik toleransi 1e-3
kind   face: plane|cylinder|cone|sphere|torus|other   tepi: line|circle|other
area   luas face (mm2)          len  panjang tepi (mm)
r      radius (face silinder/kerucut/bola, tepi lingkaran)
x y z  komponen centroid face / mid tepi
Hasil terurut naik. Kosong -> selector_empty + context.available (jumlah per jenis).
CONTOH
>Z                          face paling atas
+Z[z=5]                     face menghadap +Z pada ketinggian 5
#Z                          dinding samping box
|Z                          tepi tegak (untuk fillet sudut)
of(>Z)                      tepi keliling face atas
of(>Z) and |X               tepi atas yang sejajar X
all except |Z               semua tepi selain yang tegak
all[kind=cylinder][r=2.75]  dinding lubang M5 clearance
";

pub use eval::{eval_edges, eval_faces, ANG_TOL_DEG};
pub use parse::{parse, SelCtx, SelExpr};

use ducad_kernel::KernelShape;

use crate::error::{OpErrorCode, OpResult};

/// Ganti selector di pesan `SelectorEmpty` dengan teks aslinya (evaluator
/// hanya melihat AST).
fn with_source(mut e: crate::OpError, src: &str) -> crate::OpError {
    if e.code == OpErrorCode::SelectorEmpty {
        let available = e.context.get("available").cloned().unwrap_or_default();
        e = eval::empty_error(src, available);
    }
    e
}

/// Parse + enumerasi + evaluasi selector face pada `shape`.
pub fn select_faces(shape: &KernelShape, src: &str) -> OpResult<Vec<usize>> {
    let expr = parse(src, SelCtx::Face)?;
    let faces = ducad_kernel::enumerate_faces(shape);
    eval_faces(&expr, &faces).map_err(|e| {
        if e.code == OpErrorCode::SelectorEmpty {
            let edges = ducad_kernel::enumerate_edges(shape);
            return eval::empty_error(src, eval::available_summary(&faces, Some(&edges)));
        }
        e
    })
}

/// Parse + enumerasi + evaluasi selector tepi pada `shape`.
pub fn select_edges(shape: &KernelShape, src: &str) -> OpResult<Vec<usize>> {
    let expr = parse(src, SelCtx::Edge)?;
    let faces = ducad_kernel::enumerate_faces(shape);
    let edges = ducad_kernel::enumerate_edges(shape);
    eval_edges(&expr, &faces, &edges).map_err(|e| with_source(e, src))
}
