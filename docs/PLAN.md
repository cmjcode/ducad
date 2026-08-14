# Rencana Implementasi CADRAW

Aplikasi CAD gaya AutoCAD (drafting 2D presisi) yang berevolusi ke pemodelan
3D gaya Shapr3D (sketch → extrude, direct modeling). Rust + egui/eframe,
kernel geometri OpenCASCADE via [opencascade-rs](https://github.com/bschwind/opencascade-rs).
Target desktop (macOS/Windows/Linux) dan iPad.

## Keputusan Arsitektur

- **Workspace Cargo**, satu crate per lapisan tanggung jawab — lihat tabel di bawah.
- **Kernel**: OpenCASCADE (via `opencascade-rs`), dibungkus di `cadraw-kernel`.
  Seluruh aplikasi hanya menyentuh tipe dari `cadraw-kernel`, tidak pernah
  `opencascade`/OCCT langsung — supaya kernel bisa ditambal/diganti tanpa
  merombak app. (truck dipertimbangkan dan ditolak: tidak punya
  fillet/chamfer 3D dan boolean rapuh di kasus tangensial — lihat diskusi
  di riwayat perencanaan.)
- **Rendering**: viewport 3D custom via `egui_wgpu` paint callback. Mesh
  hasil tessellation OCCT (`BRepMesh`) di-upload ke wgpu, di-cache per-body.
- **Paradigma modeling**: direct modeling (sketch → push/pull), bukan
  feature-tree parametrik penuh — realistis untuk kernel & cocok untuk touch.
- **Input abstraction**: event pointer yang sama untuk mouse, jari, dan
  Apple Pencil, sejak Fase 0 — supaya tool otomatis jalan di kedua platform.
- **Undo/redo**: command pattern (`cadraw_core::Command`) sejak hari pertama.

## Struktur Workspace

| Crate | Peran |
|---|---|
| `cadraw-core` | Document model, command/undo-redo |
| `cadraw-sketch` | Entitas 2D, snapping, constraint solver (Fase 1–2) |
| `cadraw-kernel` | Wrapper di atas opencascade-rs (modeling, boolean, mesh, STEP) |
| `cadraw-render` | Viewport wgpu: kamera orbit, grid, pipeline, gizmo |
| `cadraw-io` | Format native `.cadraw`, STEP, DXF, STL/OBJ (Fase 5) |
| `cadraw-ui` | Komponen egui bersama: toolbar, radial menu, numpad (Fase 4) |
| `cadraw-app` | Shell eframe desktop; basis entry point iOS (Fase 6) |

## Fase

0. **Fondasi & viewport** — workspace, kamera orbit/pan/zoom, grid, spike
   build iOS + spike cross-compile OCCT ke `aarch64-apple-ios`. **[status:
   kerangka jadi, lihat bawah]**
1. **Sketching 2D + snapping** — line/arc/circle/spline, snap engine,
   dynamic input.
2. **Constraint solver** — coincident/parallel/tangent/dst, solver numerik
   Newton/LM ditulis sendiri di `cadraw-sketch`.
3. **Modeling 3D** — extrude/revolve/sweep, boolean, sketch-on-face, fillet/
   chamfer/shell sebagai fitur inti (bukan "sejauh kemampuan kernel").
4. **UX shell** — toolbar kontekstual, command palette, radial menu iPad,
   target sentuh ≥44pt, tema.
5. **File I/O** — `.cadraw` native (serde+versioning), STEP, DXF, STL/OBJ.
6. **Port iPad** — winit iOS + Metal via wgpu, Apple Pencil, Files.app,
   TestFlight.
7. **Poles & performa** — alat ukur, section view, tessellation di thread
   terpisah, packaging.

Detail penuh tiap fase, tabel risiko, dan estimasi ada di riwayat sesi
perencanaan (`/plan` awal). Ringkasan risiko tertinggi:

- Cross-compile OCCT ke iOS + ukuran binary — dibuktikan lewat spike di
  Fase 0, bukan ditunda ke Fase 6.
- Dukungan egui/winit di iOS masih berkembang — spike sama.
- Coverage binding `opencascade-rs` belum 100% API OCCT — mitigasi: tambah
  binding cxx sendiri di `cadraw-kernel` saat dibutuhkan.

## Status Fase 0 (dikerjakan)

- [x] Workspace 7 crate ter-scaffold, `cargo build -p cadraw-app` hijau,
      unit test kamera (`cadraw-render::camera`) dan undo-stack
      (`cadraw-core`) lulus.
- [x] Viewport wgpu: kamera orbit turntable (Z-up, tanpa roll), grid XY
      minor/mayor + sumbu berwarna, shader WGSL dasar (mesh + garis dengan
      fade jarak).
- [x] Navigasi: drag kiri/tengah orbit, shift+drag/kanan pan, scroll/pinch
      zoom, multi-touch dua jari (trackpad/iPad) untuk orbit+zoom.
- [x] `cadraw-kernel`: wrapper `make_filleted_box`/`tessellate`/`write_stl`
      di atas opencascade-rs, plus binary `smoke` untuk validasi.
- [x] Build OCCT (`cargo build -p cadraw-kernel`) — sukses. Sempat kena
      konflik versi `glam` (opencascade 0.2.0 pin ke glam 0.23, workspace
      pakai 0.29) — diperbaiki dengan memberi `cadraw-kernel` dependensi
      glam 0.23 sendiri (tidak dari workspace), karena `KernelMesh`
      memang sudah mengonversi ke `[f32; 3]` mentah sehingga tidak
      membocorkan tipe glam versi berbeda ke crate lain.
- [x] Smoke test kernel (`cargo run -p cadraw-kernel --bin smoke`) — box
      40×30×20 mm + fillet r3 → 2129 vertex/3478 tri, STL tertulis.
      Alur inti "sketch → extrude → fillet → mesh → export" terbukti hidup.
- [ ] Verifikasi visual jendela: **perlu dijalankan manual** oleh
      developer (`cargo run -p cadraw-app`) di sesi desktop interaktif —
      shell agent tidak punya akses WindowServer untuk screenshot.
- [ ] Spike build iOS (winit iOS + cross-compile OCCT) — belum dimulai,
      masih prioritas tertinggi berikutnya (risiko arsitektur, lihat
      status Fase 1 di bawah untuk kenapa ditunda satu putaran).

## Status Fase 1 (dikerjakan)

- [x] `cadraw-core::Command`/`UndoStack` digeneralisasi jadi generik atas
      target `T` (bukan cuma `Document`), supaya `cadraw-sketch` bisa pakai
      undo/redo yang sama tanpa retrofit. `Document` (3D) tetap jalan lewat
      `Command<Document>`.
- [x] `cadraw-sketch`: entitas Line/Circle/Arc, hit-testing (jarak titik-ke-
      entitas), snap engine dengan prioritas endpoint > midpoint > center >
      intersection > grid, command `InsertEntities`/`DeleteEntities`
      (undo-able). 5 unit test lulus (hit-test, 3 skenario snap,
      insert/delete roundtrip).
- [x] `cadraw-render`: modul `sketch` mengonversi entitas + preview + glyph
      snap (bentuk beda per jenis: kotak endpoint, segitiga midpoint,
      lingkaran center, diamond intersection, silang grid) jadi
      `LineVertex`; `SceneRenderer` dapat buffer overlay dinamis yang
      di-upload ulang tiap frame lewat pipeline garis yang sama dengan grid.
- [x] `cadraw-app`: tool Pilih/Garis/Persegi/Lingkaran lengkap — klik 2 titik
      dengan snap otomatis, preview rubber-band, dynamic input (ketik
      panjang/radius + Enter, gaya AutoCAD), seleksi klik & Shift+klik,
      hover highlight, Delete/Backspace hapus, Ctrl/Cmd+Z undo, Ctrl/Cmd+
      Shift+Z atau Ctrl+Y redo, shortcut L/R/C ganti tool, Esc batal/kembali
      ke Pilih. Ray-plane picking (unproject NDC → intersect Z=0) dan
      toleransi hit-test berbasis piksel-ke-dunia pada jarak kamera.
      `cargo build`/`clippy -D warnings`/`test` semua hijau di seluruh
      workspace (9 test lulus: 3 kamera, 1 undo-core, 5 sketch).
- [ ] **Sengaja belum ada di putaran pertama** (lihat "Fase 1 lanjutan" di
      bawah untuk yang sudah ditambahkan): Ellipse/spline, fillet 2D,
      trim/extend, offset, mirror; interaksi drag-satu-gesture (saat ini
      dua-klik terpisah — drag-to-draw ala Shapr3D masuk Fase 4).
- [ ] Verifikasi visual & UX sketching di device sungguhan (mouse/trackpad
      dan idealnya iPad) — sama seperti Fase 0, belum bisa dicek dari
      sandbox agent.

## Status Fase 1 Lanjutan (dikerjakan)

- [x] `cadraw-sketch`: entitas `Ellipse` (axis-aligned, distance_to via
      sampling batas — tak ada rumus tertutup titik-ke-ellips), hit-test &
      snap center ikut otomatis lewat match arm yang sudah ada.
- [x] `arc_from_three_points(p1, p2, p3)`: bangun Arc lewat circumcenter +
      penentuan CCW start/end yang benar berdasar posisi `p2`. Dites lulus
      untuk kasus lurus (`None`, kolinear) dan kasus valid (verifikasi
      ketiga titik berjarak sama ke center + p2 ada di rentang sudut).
- [x] `offset_entity(entity, reference_point)`: hasil offset ditentukan
      langsung dari satu titik klik (jarak + sisi sekaligus) — Line via
      proyeksi normal bertanda, Circle/Arc via jarak ke center. Ellipse
      sengaja `None` (parallel-curve ellips sejati bukan ellips lagi,
      tidak direpresentasikan model axis-aligned kita — didokumentasikan,
      bukan pendekatan yang salah).
- [x] `mirror_entity(entity, axis_a, axis_b)`: refleksi titik generik untuk
      Line/Circle/Ellipse (radius/rx/ry dipertahankan), plus penanganan
      Arc yang menukar start/end angle karena refleksi membalik arah CCW.
      Catatan keterbatasan didokumentasikan di kode: Ellipse hasil mirror
      cuma presisi untuk sumbu cermin horizontal/vertikal (ellips
      berotasi belum didukung model).
- [x] `trim_segments`/`project_t`/`line_intersection_params_in_sketch`:
      Trim Line-vs-Line — klik sub-segmen di antara/di luar titik potong
      untuk menghapusnya, sisa 0-2 potongan disisipkan lewat command baru
      `ReplaceEntities` (hapus+sisip sebagai satu langkah undo).
      16 unit test `cadraw-sketch` lulus total (11 baru di putaran ini).
- [x] `cadraw-render`: render Ellipse (tessellation parametrik rx/ry
      independen) dan `removal_preview_lines` (warna peringatan merah
      untuk pratinjau segmen yang akan terhapus Trim).
- [x] `cadraw-app`: 5 tool baru — Ellips (E, 2-klik kotak pembatas), Arc
      (A, 3-klik: awal/akhir/titik-di-busur, preview live begitu 2 titik
      terisi), Offset (O, klik sumber lalu klik sisi+jarak, preview live),
      Mirror (M, perlu seleksi non-kosong dari tool Pilih lebih dulu, 2
      klik sumbu cermin dengan preview ghost semua entitas terpilih),
      Trim (T, hover menyorot merah sub-segmen yang akan hilang, klik
      commit). Toolbar dirapikan dengan `horizontal_wrapped` supaya 9
      tombol tool tidak terpotong. `pending_first: Option<DVec2>` Fase 1
      digeneralisasi jadi `pending_points: Vec<DVec2>` supaya tool 2-titik
      dan 3-titik (Arc) berbagi jalur commit yang sama
      (`on_click_point`/`finish_multipoint`).
      Seluruh workspace hijau: `build`/`clippy -D warnings`/`test` (20
      test: 3 kamera, 1 undo-core, 16 sketch).
- [ ] **Sengaja belum ada** (lihat juga daftar Fase 1 pertama di atas):
      spline, fillet 2D (round corner dengan tangency), extend, offset
      untuk Ellipse, dynamic input untuk Ellipse/Arc/Offset/Mirror/Trim
      (baru Line/Rectangle/Circle), toleransi snap adaptif mouse-vs-sentuh
      presisi, drag-satu-gesture. Trim juga hanya menghitung potongan
      Line-vs-Line (belum Line-vs-Circle/Arc), dan hit-test tool Trim
      memfilter hasil `hit_test` global ke Line setelahnya (bukan
      hit-test khusus per-jenis) — kadang meleset kalau ada entitas
      non-Line yang lebih dekat dari Line terdekat; jarang terasa dalam
      pemakaian normal (klik langsung di garis), dicatat sebagai
      penyederhanaan yang bisa diperbaiki nanti bukan bug tersembunyi.
- [ ] Verifikasi visual & UX tool-tool baru di device sungguhan — sama
      seperti sebelumnya, belum bisa dicek dari sandbox agent.

## Menjalankan

```bash
# App desktop (viewport 3D)
cargo run -p cadraw-app

# Smoke test kernel OCCT (setelah build pertama selesai)
cargo run -p cadraw-kernel --bin smoke

# Unit test
cargo test --workspace
```
