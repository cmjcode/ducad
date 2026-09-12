# Status Fase A (P0), B (P1), C (P2) & D (P4)

Diperbarui: 2026-09-12. Sumber rencana: `.claude/plans/ducad-pro-cad-roadmap.plan.md`.

Estimasi awal untuk Fase A + B adalah **6–8 bulan kerja penuh satu engineer
senior**. Dokumen ini mencatat apa yang sudah benar-benar mendarat dan
terverifikasi, versus apa yang belum — bukan rencananya.

Gate mutu di tiap baris "selesai": `cargo clippy --workspace --all-targets
-- -D warnings` exit 0 DAN `cargo test --workspace` hijau.

---

## Fase A — P0 Fondasi

| Item | Status | Catatan |
|---|---|---|
| **P0.6** CI & quality gate | ✅ **selesai** | `.gitmodules` (repo sebelumnya tak bisa di-clone), workflow 4 lane, clippy `-D warnings` memblokir, profil rilis. `fmt`/`deny` sengaja belum memblokir — lihat ADR 0001. |
| **P0.2** Undo lintas domain | 🟡 **sebagian** | `Transaction`, coalescing drag, batas kedalaman, `begin`/`commit`/`rollback` — selesai & teruji di `ducad-core`. **Belum**: transaksi yang benar-benar melintasi domain, karena `Sketch` dan `ModelDoc` masih dua target `Command<T>` terpisah (menunggu P0.1). |
| **P0.1** Model dokumen tunggal | 🟡 **sebagian** | `SketchSet` berkunci `SketchId` + `PlaneRef` menggantikan tiga larik paralel; N sketsa per bidang kini mungkin. **Belum**: `Document` tunggal (fitur, datum, parameter, assembly, drawing), pemisahan `EditorState`, crate `ducad-editor-core` headless. |
| **P0.3** Format native v2 | 🟡 **sebagian** | `KernelShape::to_brep_bytes`/`from_brep_bytes` — 3,6× lebih kecil dari teks STEP, roundtrip terverifikasi. **Belum**: container ZIP, penyimpanan DAG/datum/assembly/drawing, migrasi v1→v2, autosave/recovery. |
| **P0.4** Topological naming | ❌ **belum** | Butuh binding cxx baru: `BRepTools_History`, `BRepBuilderAPI_MakeShape::Generated/Modified/IsDeleted`. **Memblokir P0.5.** |
| **P0.5** Regenerasi parametrik sungguhan | ❌ **belum** | Bergantung P0.4. `regenerate_parametric_model` masih meng-extrude semua entitas di bidang, memakai `fillet_all`, memetakan fitur→body lewat urutan indeks, dan melewati Boolean/Hole/Helix/Sweep/Loft. |

### Hutang teknis yang diketahui dan sengaja dibiarkan

- `parametric_engine` masih menyimpan `plane_index: usize` di payload fitur.
  Indeks itu bergeser saat datum plane dihapus — persis cacat yang
  `PlaneRef` hilangkan. Konversinya sudah dipusatkan di
  `document.rs::plane_ref_for_index`; memindahkan payload ke `PlaneRef`
  adalah bagian P0.5.
- `DuCADApp` masih ~96 field.

---

## Fase B — P1 Sketch & Drafting 2D

| Item | Status | Catatan |
|---|---|---|
| **P1.4** Nested region → profil berlubang | ✅ **selesai** | `find_region_hierarchy` (aturan ganjil-genap), `Profile::WithHoles`, `build_face_on_plane`. Plus `KernelShape::volume()` (B-rep eksak) yang dibutuhkan untuk membuktikannya. |
| **P1.2** Solver | 🟡 **sebagian** | `analyze_dof` (rank, DOF, redundansi **per indeks kendala**), dekomposisi gugus union-find, 4 constraint baru (`PointOnCurve`, `Midpoint`, `Concentric`, `Collinear`). **Belum**: Jacobian analitik, sparse solver, drag-with-solver, auto-constraint inference, pewarnaan DOF di kanvas. |
| **P1.7** DXF | 🟡 **sebagian** | Impor LWPOLYLINE (terbuka/tertutup), POLYLINE gaya lama, segmen bulge→busur, ELLIPSE sejajar sumbu. **Belum**: SPLINE, TEXT/MTEXT, INSERT/BLOCK, HATCH, DIMENSION, tabel LAYER/LTYPE, DXF biner, ekspor entitas baru, DWG. |
| **P1.1** Entitas 2D baru | ❌ **belum** | `Point`, `Polyline` (bulge), NURBS `BSpline`, `Ellipse` berotasi, `Text` sebagai entitas, `Hatch`. Menyentuh hit-test, snap, offset, trim, render, dan kernel. **Ellips berotasi memblokir impor DXF-nya** (saat ini dilewati, bukan diimpor salah). |
| **P1.6** Command line ala AutoCAD | ❌ **belum** | Alias perintah, `@10<45`, Ortho/Polar/Otrack, Move/Copy/Rotate/Scale/Stretch/Array/Join/Explode/Break. |
| **P1.3** Driving dimensions + parameter | ⛔ **terblokir** | Butuh `ParamTable` di `Document` — menunggu P0.1. |
| **P1.5** Layers, blocks, linetypes | ⛔ **terblokir** | Butuh `Document` — menunggu P0.1. |

---

## Fase C — P2 Pemodelan 3D

| Item | Status | Catatan |
|---|---|---|
| **P2.7** Robustness kernel | 🟡 **sebagian** | `validate_or_heal` (periksa → `ShapeFix` → periksa lagi → gagal) dipasang di Union/Subtract/Intersect/Fillet/Chamfer. Binding cxx baru `BRepCheck_Analyzer` + `ShapeFix_Shape`. **Belum**: boolean fuzzy (`SetFuzzyValue`), pelaporan edge mana yang menggagalkan fillet, dan mengganti 224 `unwrap()` dengan `thiserror`. |
| **P2.6** Mass properties | 🟡 **sebagian** | `volume()` dan `surface_area()` eksak dari B-rep. **Belum**: massa dari densitas material, pusat massa, tensor inersia, panel UI-nya. Sisa P2.6 (hole ANSI, cosmetic thread, pattern regen-aware, sheet metal) belum. |
| **P2.1** Mode extrude | 🟡 **sebagian** | `ExtrudeExtent::{Blind, Symmetric, TwoSided}` — semuanya diselesaikan sebagai SATU prisma dengan menggeser titik awal, tanpa boolean. **Belum**: `UpToNext`/`UpToBody`/`UpToFace` (butuh `BRepFeat_MakePrism` + **P0.4**), draft, thin, opsi revolve. |
| **P2.2** Sketch on face asosiatif | ⛔ **terblokir** | Butuh `TopoRef` dari **P0.4**. |
| **P2.3** Binding OCCT batch | 🟡 **sebagian** | 3 dari ~17 kelas terikat (`BRepCheck_Analyzer`, `ShapeFix_Shape`, `SurfaceProperties`). **Belum** yang paling penting: `BRepTools_History` (P0.4) dan `HLRBRep_Algo` (Fase D). |
| **P2.4** Surface modeling | ❌ **belum** | Thicken, knit, trim, extend, replace/delete face. |
| **P2.5** Direct modeling | ❌ **belum** | Move/rotate/offset/delete/replace face, combine multi-body, scale. |
| **P2.8** Kernel paralel & tessellation inkremental | ❌ **belum** | `KERNEL_LOCK` masih menyerialkan SEMUA operasi OCCT, bukan hanya transfer STEP/IGES. |

---

## Fase D — P4 Gambar Kerja Teknik

| Item | Status | Catatan |
|---|---|---|
| **P4.1** HLR eksak | 🟡 **sebagian** | `hlr_exact.rs` di atas binding cxx baru `HLRBRep_Algo`. Kurva mempertahankan jenis analitiknya — lingkaran tetap `EdgeType::Circle`, dan hasilnya tidak bergantung kerapatan tesselasi. **Belum**: menggantikan pemakaian `hlr.rs` lama di `drawing_sheet_view`/ekspor (itu P4.2), auxiliary view, dan proyeksi 6 tampak. |
| **P4.2** Model sheet & view | ❌ **belum** | Multi-sheet, `DrawingView` (Projected/Auxiliary/Section/Detail/Broken/Crop), penyelarasan tampak, template title block kustom. `hlr.rs` berbasis mesh masih dipakai di sini. |
| **P4.3** Anotasi standar | ⛔ **terblokir** | Dimensi asosiatif butuh `TopoRef` dari **P0.4**. GD&T, toleransi ISO 286, surface finish, weld belum. |
| **P4.4** Ekspor gambar | ❌ **belum** | DXF dengan entitas DIMENSION asli, PDF/A multi-halaman dengan font tertanam. Kini MUNGKIN menulis CIRCLE/ARC sungguhan berkat P4.1, tapi butuh P4.2 dulu. |

---

## Urutan yang disarankan berikutnya

1. **P0.4** binding `BRepTools_History` — kini penghalang terbesar:
   memblokir P0.5 (regen parametrik), P2.2 (sketch on face asosiatif), dan
   `UpToFace` di P2.1. Tanpanya fillet/hole/mate tidak akan pernah
   asosiatif. Polanya sudah terbukti: dua binding cxx baru ditambahkan di
   P2.7 tanpa perlu menautkan pustaka OCCT baru.
2. **P0.1 lanjutan** — `Document` tunggal + `EditorState`. Membuka P1.3 dan
   P1.5 sekaligus, dan prasyarat container v2 (P0.3).
3. **P1.1** entitas 2D — juga membuka sisa impor DXF (ellips berotasi saat
   ini dilewati, bukan diimpor salah).

## Tindakan yang perlu pemilik repo

- **Push submodule**: `git -C ducad-editor/vendors/opencascade-rs push
  origin ducad-patches`. Tanpa ini gitlink menunjuk commit yang tidak ada
  di remote dan CI gagal meng-clone.
- **Keputusan reformat rustfmt massal** (~1.628 selisih). Prosedurnya ada
  di `.github/workflows/ci.yml`.
