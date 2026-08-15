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
   dynamic input. **[status: selesai + iterasi lanjutan, lihat bawah]**
2. **Constraint solver** — coincident/parallel/tangent/dst, solver numerik
   Newton/LM ditulis sendiri di `cadraw-sketch`. **[status: selesai — 12
   jenis constraint + UI lengkap termasuk pemilihan titik, lihat bawah]**
3. **Modeling 3D** — extrude/revolve/sweep, boolean, sketch-on-face, fillet/
   chamfer/shell sebagai fitur inti (bukan "sejauh kemampuan kernel").
   **[status: putaran pertama selesai — Extrude, Union/Subtract, Fillet/
   Chamfer semua tepi, Shell/Hollow, render mesh 3D nyata; Revolve/loft/
   intersect/fillet-chamfer-per-tepi/shell-multi-face DITUNTASKAN Fase 8;
   sweep & sketch-on-face masih ditunda, lihat status Fase 8 di bawah]**
4. **UX shell** — toolbar kontekstual, command palette, radial menu iPad,
   target sentuh ≥44pt, tema. **[status: putaran pertama selesai — command
   palette, radial menu long-press, toggle tema, target sentuh global,
   lihat bawah]**
5. **File I/O** — `.cadraw` native (serde+versioning), STEP, DXF, STL/OBJ.
   **[status: putaran pertama selesai — save/load native, import/export
   STEP & DXF, export STL/OBJ, lihat bawah]**
6. **Port iPad** — winit iOS + Metal via wgpu, Apple Pencil, Files.app,
   TestFlight. **[status: seluruh stack Rust CADRAW + eframe/winit/wgpu
   terbukti compile bersih untuk `aarch64-apple-ios`; satu blocker upstream
   tersisa — OCCT (kernel geometri) belum bisa link untuk iOS, lihat
   bawah]**
7. **Poles & performa** — alat ukur, section view, tessellation di thread
   terpisah, packaging. **[status: putaran pertama selesai — tool Ukur
   Jarak/Sudut, Section View (clip plane shader), worker thread Import
   STEP + kunci kernel global baru, metadata packaging `cargo-bundle`,
   lihat bawah]**
8. **Modeling 3D lanjutan** — Revolve, loft, boolean intersect, picking
   edge/face 3D di viewport untuk fillet/chamfer per-tepi & shell
   multi-face (kekurangan terbesar yang ditunda sejak Fase 3).
   **[status: putaran pertama selesai — Revolve 360°, Loft 2-profil,
   Boolean Intersect, infrastruktur picking edge/face berbasis ray-dunia
   (bukan index — lihat desain kunci di bawah), Fillet/Chamfer per-tepi,
   Shell multi-face; sweep & sketch-on-face ditunda, lihat bawah]**

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

## Status Fase 2 — Constraint Solver (dikerjakan)

- [x] `cadraw-sketch::constraint`: parametrisasi entitas → vektor unknown
      f64 (Line 4 DOF, Circle 3, Arc 5, Ellipse 4), 12 jenis constraint
      (Coincident, Horizontal, Vertical, Parallel, Perpendicular,
      EqualLength, EqualRadius, Fixed, Distance, Radius, Angle, Tangent,
      Symmetric) — cukup untuk mayoritas kebutuhan sketch sehari-hari.
- [x] Solver **Levenberg-Marquardt ditulis sendiri** (bukan library
      eksternal): Jacobian finite-difference, eliminasi Gauss + pivot
      parsial untuk normal equations, damping `lambda·I` klasik (bukan
      diskalakan diagonal JtJ — lihat catatan bug di bawah).
      13 unit test lulus: satu per jenis constraint (verifikasi hasil
      solve benar secara geometris, bukan cuma "tidak crash"), kasus
      Fixed+Horizontal berbarengan, kasus constraint saling bertentangan
      (dua Fixed ke titik yang sama menuju target berbeda) yang harus
      gagal konvergen tanpa panic, dan roundtrip undo/redo.
- [x] **Bug ditemukan & diperbaiki saat testing**: damping Marquardt asli
      (`lambda × diagonal(JtJ)`) membuat sistem singular setiap kali ada
      parameter yang sama sekali tak disentuh constraint manapun (mis.
      center lingkaran saat cuma constraint Radius aktif — arah bebas
      punya JtJ diagonal persis nol, jadi damping ikut nol, tidak
      meregularisasi apa-apa). Diperbaiki dengan damping Levenberg klasik
      (`lambda × I`), yang tetap meregularisasi arah bebas berapa pun
      nilai JtJ-nya. Baru ketahuan lewat 2 test yang gagal (radius-only
      dan fixed+horizontal) — bukti nyata kenapa "teruji unit" di rencana
      awal itu penting, bukan formalitas.
- [x] Command `AddConstraint`/`RemoveConstraint` (undo-able, snapshot
      geometri sebelum solve untuk revert persis).
- [x] `cadraw-app`: panel Constraint kontekstual di kanan layar, muncul
      saat tool Pilih aktif + 1-2 entitas terpilih. 1 Line → Horizontal/
      Vertikal/Panjang; 1 Circle/Arc → Radius; 2 Line → Sejajar/Tegak
      Lurus/Sama Panjang/Sudut; 2 Circle/Arc → Sama Radius. Pola "dry-run
      dulu": constraint di-solve di atas clone sketch sebelum dikirim ke
      undo stack — kalau gagal konvergen, sketch nyata TIDAK berubah sama
      sekali, cuma pesan error tampil di panel (residual sisa ditampilkan).
      Jumlah constraint aktif ditambahkan ke status bar bawah.
      Seluruh workspace hijau: `build`/`clippy -D warnings`/`test` (33
      test: 3 kamera, 1 undo-core, 29 sketch termasuk 13 constraint).
- [x] **Tangent & Symmetric ditambahkan** ("tuntaskan dulu Tangent/
      Symmetric/UI Coincident-Fixed di Fase 2"). Tangent: Line-Radial
      (jarak titik-ke-garis-tak-hingga = radius) dan Radial-Radial (jarak
      antar center = jumlah radius, tangensial eksternal saja); Line-Line
      no-op karena tak masuk akal secara geometris. Symmetric: titik `a`
      dan `b` saling cermin lintas garis `axis`, pakai `reflect_point` yang
      diekstrak jadi fungsi bersama (dipakai juga oleh `mirror_entity` yang
      sudah ada). Ditambahkan `EntityKind` (Line/Radial) yang di-snapshot
      SEKALI dari `Sketch` sebelum solve — perlu karena Tangent butuh tahu
      jenis entitas untuk memilih formula, tapi closure residual tidak
      boleh meng-capture `&Sketch` (bentrok dengan `&mut Sketch` di
      `write_back`), jadi kind dibaca duluan lalu dipegang sebagai map
      biasa. 5 test baru, termasuk verifikasi Arc (5 DOF) tidak merusak
      pembacaan offset yang dipakai bersama Circle (3 DOF).
- [x] **UI Coincident/Fixed tuntas**, dengan infrastruktur baru: `SnapHit`
      sekarang bawa `source: Option<PointRef>` — snap ke Endpoint/Center
      (bukan Midpoint/Intersection/Grid, yang bukan DOF tunggal) membawa
      rujukan persis entitas+bagian mana yang di-snap, lewat method baru
      `Entity::endpoint_refs`/`center_ref`. Tiga tool baru di `cadraw-app`:
      **CoincidentPick** (klik 2 titik via snap → berimpit), **FixedPick**
      (klik 1 titik → ditahan di posisi sekarang, tanpa perlu ketik target
      — pin di tempat adalah pemakaian paling umum), **SymmetricPick**
      (perlu 1 Line terpilih dulu sbg sumbu, pola sama seperti Mirror,
      lalu klik 2 titik). Titik yang sudah diklik ditandai marker silang
      ungu (`picked_point_glyph`, beda warna dari glyph snap oranye
      supaya "sudah dipilih" tak tertukar "sedang di-hover"). Tombol
      Tangent juga ditambahkan ke panel Constraint untuk pasangan Line+
      Radial atau Radial+Radial. `point_ref_position` (baca posisi PointRef
      langsung dari Sketch, bukan dari vektor parameter solve) jadi utilitas
      umum dipakai UI merender titik yang sudah dipilih.
      **Bug ditemukan lewat test, bukan cuma teori** (lagi): test pertama
      untuk Symmetric gagal karena asumsi keliru — constraint itu cuma
      menjamin `reflect(a) == b`, TIDAK memaksa titik `a` atau sumbu diam;
      dengan 2 residual vs 12 unknown (3 entitas × 4 DOF), solver bebas
      menggeser ketiganya bersamaan. Test diperbaiki untuk memverifikasi
      invarian yang benar-benar dijamin (reflect terhadap posisi FINAL),
      pola yang sama dipakai test Parallel/Perpendicular sebelumnya.
      Seluruh workspace hijau: `build`/`clippy -D warnings`/`test` (40
      test: 3 kamera, 1 undo-core, 36 sketch termasuk 18 constraint).
- [ ] **Masih sengaja belum ada** (Fase 2 kini lengkap sesuai lingkup yang
      ditetapkan, sisanya eksplisit Fase 4+): browser/manajer constraint
      (lihat/hapus daftar selain lewat Undo), indikator visual DOF
      (biru=bebas, hitam=fully constrained ala Shapr3D), auto-constraint
      saat menggambar (garis hampir horizontal → otomatis Horizontal),
      constraint pada titik ujung Arc (`PointRef` belum mencakupnya),
      point-on-entity (coincident ke kurva, bukan cuma titik-ke-titik),
      tangensial internal, Tangent Line-Line, dynamic input untuk tool
      pemilihan titik. Jacobian numerik (bukan analitik) — cukup cepat
      untuk skala sketch, dipertimbangkan ulang di Fase 7 kalau profiling
      menunjukkan perlu.
- [ ] Verifikasi visual & UX panel Constraint + tool pemilihan titik di
      device sungguhan — sama seperti sebelumnya, belum bisa dicek dari
      sandbox agent.

## Status Fase 3 — Modeling 3D (dikerjakan, putaran pertama)

- [x] `cadraw-kernel` ditulis ulang: `KernelShape` (pembungkus `Shape` OCCT
      yang SEPENUHNYA privat — sebelumnya `make_filleted_box`/`tessellate`
      membocorkan tipe `opencascade::primitives::Shape` langsung ke
      pemanggil, melanggar aturan arsitektur sendiri; sekarang benar-benar
      tertutup). API baru: `extrude_profile`, `union`, `subtract`,
      `fillet_all`, `chamfer_all`, `shell_hollow`, semua fungsional
      (`&KernelShape` masuk, `KernelShape` baru keluar — tidak memutasi
      input pemanggil).
- [x] `Profile`/`ProfileSegment`: profil 2D di bidang XY dalam koordinat
      mentah `(f64,f64)` (bukan `glam::DVec2`) — pola yang sama dengan
      `KernelMesh` sebelumnya, supaya glam 0.23 (pin kernel) tidak pernah
      bocor ke `cadraw-app` (glam 0.29). Mendukung `Circle` (jadi silinder)
      dan `Loop` tertutup segmen Line/Arc.
- [x] **Bug ditemukan lewat test, bukan teori** (lagi — pola yang sama
      persis dengan bug damping LM di Fase 2): `opencascade-rs` 0.2.0
      TIDAK menyediakan `Clone` untuk `Shape` (cuma `UniquePtr` C++ tanpa
      binding copy-constructor), padahal `fillet`/`chamfer` memutasi diri
      sendiri di tempat dan `hollow` mengonsumsi kepemilikan — kalau
      dipakai langsung, shape ASLI pemanggil akan rusak/hilang, merusak
      undo. Diperbaiki dengan `deep_clone` internal: roundtrip lewat file
      STEP sementara (satu-satunya cara publik menyalin B-rep persis di
      binding ini) sebelum operasi destruktif — didokumentasikan di kode
      sebagai keputusan sadar, bukan technical debt.
- [x] **Bug thread-safety ditemukan lewat test**: `cargo test -p
      cadraw-kernel` (multi-thread default) crash `SIGABRT` /
      `Interface_InterfaceError` — jalur transfer STEP OCCT (dipakai
      `deep_clone`) punya state global yang tidak aman dipanggil dari
      banyak thread sekaligus. Semua 9 test lulus satu-satu; diperbaiki
      dengan `Mutex` global yang menyerialkan test modul (tidak
      mempengaruhi `cadraw-app` — kernel selalu dipanggil dari UI thread
      tunggal).
- [x] **Blocker environment ditemukan & diperbaiki**: `cargo build -p
      cadraw-kernel` gagal total di mesin ini — CMake 4.3.4 terinstal
      menolak `cmake_minimum_required` versi lama di `CMakeLists.txt`
      bawaan OCCT (dependensi `occt-sys`). Diperbaiki lewat
      `.cargo/config.toml` (`CMAKE_POLICY_VERSION_MINIMUM = "3.5"`,
      env var yang dibaca `cmake` crate) — otomatis berlaku untuk semua
      `cargo build/test/run` di workspace ini, tidak perlu diketik manual.
      Build OCCT dari source makan waktu ~8 menit sekali (di-cache
      `target/` setelahnya).
- [x] `cadraw-app/src/model.rs` (modul baru, pola sama dengan
      `cadraw-sketch::constraint`): `ModelDoc` menggabungkan
      `cadraw_core::Document` (metadata body, sengaja tetap bebas
      dependensi kernel) dengan `SecondaryMap<BodyId, BodyGeometry>`
      (geometri kernel sungguhan) yang dikunci `BodyId` yang sama.
      Command undo-able: `AddSolidCommand` (Extrude), `ReplaceGeometryCommand`
      (Fillet/Chamfer/Shell — `apply`/`revert` identik, cuma menukar
      geometri lama↔baru), `BooleanCommand` (Union/Subtract — hapus 2 body
      input, tambah 1 body hasil; `BodyId` body yang di-restore lewat undo
      BERUBAH, konsisten dengan konvensi `DeleteEntities` di
      `cadraw-sketch`), `DeleteBodyCommand`.
- [x] `build_profile_from_selection`: bangun `Profile` kernel dari seleksi
      entitas sketch — 1 `Circle` langsung, atau ≥3 `Line`/`Arc` yang
      dirangkai lewat titik-ujungnya (toleransi 1e-6) jadi satu loop
      tertutup, urutan pemilihan bebas.
      **Bug ditemukan lewat test**: chain-builder awal cuma tumbuh dari
      ekor (append) — kalau segmen PERTAMA yang diambil dari `HashSet`
      (urutan tak terjamin) kebetulan segmen di TENGAH rantai TERBUKA,
      pencarian sepihak salah melaporkan "tidak tersambung" alih-alih
      "tidak tertutup". Diperbaiki jadi tumbuh dari DUA ujung (append di
      ekor, prepend di kepala) — test `build_profile_open_chain_errors`
      yang awalnya gagal sekarang lulus konsisten (diverifikasi 8x jalan
      berturut-turut untuk menyingkirkan keberuntungan urutan HashSet).
- [x] `cadraw-app`: panel "Model 3D" (kiri layar, berdampingan dengan
      panel Constraint di kanan) — daftar body (checkbox visible, klik
      pilih/Ctrl+klik multi-pilih), Extrude dari seleksi sketch (input
      jarak), Union/Subtract (butuh persis 2 body terpilih), Fillet/
      Chamfer semua tepi & Shell/Hollow (butuh 1 body, dropdown arah
      untuk Shell), Hapus Body. Pola "dry-run dulu" yang sama dengan
      Fase 2: operasi dihitung dulu, cuma masuk undo stack kalau sukses;
      gagal → `model_status` tampil, `model` tak tersentuh. Undo/redo
      Model SENGAJA terpisah dari undo sketch (tombol sendiri di panel,
      bukan Ctrl+Z global) — digabung baru kalau ada kebutuhan nyata.
- [x] Render mesh 3D nyata: `cadraw-render::SceneRenderer` sudah punya
      pipeline mesh sejak Fase 0 (`set_mesh`) tapi belum pernah dipanggil
      dari app. Sekarang `CadrawApp::build_combined_body_mesh` menggabung
      mesh semua body `visible` jadi satu buffer (indeks digeser per body)
      tiap frame, diupload lewat `ViewportCallback`. `set_mesh` ditambah
      guard early-return saat kosong (wgpu menolak buffer ukuran 0) — pola
      yang sama dengan `set_overlay_lines`.
      Seluruh workspace hijau: `build`/`clippy -D warnings`/`test` (53
      test: 3 kamera, 1 undo-core, 9 kernel, 36 sketch termasuk 18
      constraint, 4 model chain-builder).
- [ ] **Sengaja belum ada** (lingkup Fase 3 dipersempit ke inti yang bisa
      dikirim solid dalam satu putaran — sisanya bukan lupa): Revolve,
      sweep/loft (API `opencascade-rs` 0.2.0 cuma punya `Solid::loft`
      lintas-penampang, bukan sweep-sepanjang-jalur sungguhan), boolean
      intersect/irisan (binding cuma expose union & subtract), sketch-on-
      face (sketch CADRAW masih selalu di bidang XY — butuh picking face
      3D + workplane lokal, infrastruktur belum ada), picking body/face
      lewat klik viewport 3D (body dipilih dari daftar di panel, bukan
      klik langsung), fillet/chamfer PER-TEPI (baru "semua tepi
      sekaligus" — perlu UI picking edge 3D), shell multi-face, undo
      gabungan Sketch+Model dalam satu stack.
- [ ] Verifikasi visual & UX panel Model + render mesh 3D di device
      sungguhan — sama seperti fase-fase sebelumnya, belum bisa dicek
      dari sandbox agent (app dicoba jalan 6 detik tanpa panic startup,
      tapi tidak ada akses WindowServer untuk screenshot).

## Status Fase 4 — UX Shell (dikerjakan, putaran pertama)

- [x] `cadraw-ui` diisi pertama kali (sebelumnya kosong sejak Fase 0):
      3 modul platform-agnostic (cuma bergantung `egui`, tidak menyentuh
      state `cadraw-app`) — `theme` (mode terang/gelap + gaya target-sentuh
      global), `command_palette` (`CommandPalette`, generik atas daftar
      `(label, hint)` yang disuplai caller tiap frame, return index balik
      ke daftar yang sama), `radial_menu` (`RadialMenu`, pola sama). Dipilih
      generik-atas-index (bukan generik atas tipe aksi lewat `Box<dyn Fn>`)
      supaya crate ini tetap tanpa dependensi ke `cadraw-sketch`/
      `cadraw_kernel`/dst — cocok dipakai ulang shell iPad Fase 6.
- [x] **Tema**: `cadraw_ui::apply_theme(ctx, ThemeMode)` — bangun
      `egui::Style` baru dari `Style::default()` (bukan mutasi style lama
      context) supaya idempoten dipanggil berkali-kali saat toggle, tidak
      menumpuk penyesuaian dari panggilan sebelumnya. Default: `Dark`.
      Tombol toggle ada di menu "⚙ Pengaturan" (lihat bawah) + entri "Ganti
      Tema" di command palette — BUKAN tombol lepas di toolbar utama lagi
      (revisi setelah putaran pertama: user minta tema & pembuka command
      palette dipindah ke satu menu Pengaturan, bukan menumpuk toolbar).
- [x] **Target sentuh ≥44pt**: `style.spacing.interact_size.y` diset sekali
      secara global di `apply_theme` — jadi lantai tinggi baris untuk semua
      widget interaktif standar egui (Button/Checkbox/ComboBox/
      SelectableLabel/dst) di SELURUH aplikasi (toolbar, panel Constraint,
      panel Model 3D, command palette), tanpa perlu disentuh manual di
      tiap situs pemanggilan widget.
- [x] **Command palette**: `Ctrl/Cmd+K` (dicek langsung di `update()`, bukan
      di `handle_sketch_input` yang menahan shortcut huruf saat ada widget
      teks fokus — supaya tetap jalan walau fokus sedang di kotak cari
      palette sendiri). Filter substring case-insensitive (bukan fuzzy
      sungguhan — cukup untuk belasan aksi CADRAW saat ini, dicatat sebagai
      batasan sadar bukan lupa). `CadrawApp::palette_actions` membangun
      daftar aksi tiap frame (murah) dari `PaletteAction` enum: ganti tool
      apa pun (termasuk 3 tool titik), Undo/Redo sketch, Undo/Redo Model,
      Ganti Tema, Hapus Seleksi (muncul kondisional, cuma kalau ada
      seleksi).
- [x] **Radial menu** (khusus tool Pilih, ditujukan untuk sentuh/iPad):
      deteksi long-press ditulis di `cadraw-app::handle_radial_menu` (bukan
      di `RadialMenu` itu sendiri, yang cuma tahu cara gambar+proses
      drag-lepas — deteksi butuh akses ke tool aktif & response viewport).
      Tekan primer diam ≥0.42 detik (toleransi gerak 6px, kalau lewat
      dianggap drag/orbit biasa dan dibatalkan) → `RadialMenu::open_at` di
      titik tekan, lalu geser ke salah satu dari 8 slice tool sketch
      (Garis/Persegi/Lingkaran/Ellips/Arc/Offset/Mirror/Trim) dan lepas
      untuk pindah tool; lepas di zona mati tengah atau Esc untuk batal.
      Orbit kamera primer dimatikan selama radial terbuka/sedang dideteksi
      (`radial_active` flag di `viewport()`) supaya drag-ke-slice tidak
      ikut memutar kamera. `radial_suppress_click` (dikonsumsi sekali per
      frame lewat `mem::take` di awal `handle_sketch_input`, sebelum early
      return apa pun) mencegah `response.clicked()` dari pelepasan pointer
      yang sama ikut diproses sebagai klik seleksi biasa saat long-press
      tidak bergerak sama sekali.
- [x] **Menu "⚙ Pengaturan"** (revisi setelah putaran pertama, user: "Theme
      dan Keyboard shortcut itu dibuat di dalam menu settings aja"):
      `CadrawApp::settings_menu` — `menu_button` di ujung kanan toolbar
      mengumpulkan toggle tema, tombol "⌘K Buka Command Palette" (sama
      efeknya dengan menekan Ctrl/Cmd+K), dan `egui::CollapsingHeader`
      "Pintasan Keyboard" berisi `egui::Grid` daftar semua shortcut huruf
      tunggal + kombinasi Ctrl/Cmd (`KEYBOARD_SHORTCUTS` const, 13 entri) —
      referensi baca-saja, BUKAN pengaturan yang bisa di-remap. Ketiganya
      dipindah dari tombol lepas di toolbar utama karena jarang disentuh
      lebih dari sekali per sesi, beda dengan tool sketch yang dipakai
      terus-menerus.
- [x] **Toolbar kontekstual** (perbaikan kecil, bukan rombak total —
      toolbar linear tetap yang utama untuk mouse/trackpad): 3 tool titik
      (Coincident/Fixed/Symmetric — dipakai jauh lebih jarang dari 9 tool
      sketch inti) dikumpulkan dari deretan tombol terpisah jadi satu
      `menu_button` "Titik ▾" yang labelnya berubah menampilkan tool titik
      aktif (mis. "● Fixed (titik)") supaya statusnya tetap kelihatan walau
      menu tertutup.
      Seluruh workspace hijau: `build`/`clippy -D warnings`/`test` (53
      test — sama seperti akhir Fase 3, Fase 4 murni UI/interaksi jadi
      tidak menambah unit test baru; diverifikasi manual lewat smoke-run
      `cargo run -p cadraw-app` 6 detik tanpa panic).
- [ ] **Sengaja belum ada** (lingkup Fase 4 dipersempit ke inti yang bisa
      dikirim dalam satu putaran): toolbar kontekstual PENUH (mis.
      tombol/panel yang benar-benar hilang-muncul mengikuti tool aktif,
      bukan cuma satu grup dikumpulkan ke menu), radial menu untuk konteks
      selain ganti tool (mis. aksi Model 3D atau constraint cepat), fuzzy
      search sungguhan di command palette (baru substring), command
      palette/radial menu belum extensible dari luar `cadraw-app` (list
      aksi & tool masih hardcoded di `main.rs`, wajar untuk single-app tapi
      perlu direvisi kalau shell iPad Fase 6 butuh daftar berbeda), deteksi
      tema sistem otomatis (cuma toggle manual), radial menu belum dites
      gesture sentuh sungguhan (long-press lewat mouse-hold di sandbox
      cuma mensimulasikan, belum tentu berperilaku identik dengan sentuh
      jari asli — perlu verifikasi device Fase 6).
- [ ] Verifikasi visual & UX (command palette, radial menu, tema, toolbar
      "Titik") di device sungguhan — sama seperti fase-fase sebelumnya,
      belum bisa dicek dari sandbox agent (tidak ada akses WindowServer
      untuk screenshot, apalagi gesture sentuh sungguhan untuk radial
      menu).

## Status Fase 5 — File I/O (dikerjakan, putaran pertama)

- [x] **`Sketch`/`Entity`/`Constraint`/`PointRef` (di `cadraw-sketch`)
      di-derive `Serialize`/`Deserialize` LANGSUNG** — bukan struct
      salinan di `cadraw-io` (satu sumber kebenaran bentuk data). Ini
      cuma mungkin karena `slotmap` di-build dengan fitur "serde"
      (ditambahkan di workspace `Cargo.toml`): `EntityId` (hasil
      `new_key_type!`) otomatis dapat `Serialize`/`Deserialize`, DAN
      `SlotMap<EntityId, Entity>` di-roundtrip apa adanya (index+versi
      internal ikut tersimpan) — jadi `EntityId` mentah di dalam
      `Constraint`/`PointRef` (mis. `Coincident { a, b }`) balik PERSIS
      sama tanpa remapping id manual sama sekali. `glam` juga dibangun
      dengan fitur "serde" (cuma di dependensi WORKSPACE 0.29 — pin
      independen `cadraw-kernel` ke glam 0.23 tidak tersentuh) supaya
      `DVec2` di dalam `Entity`/`Constraint` ikut serialize.
- [x] `cadraw-kernel`: `KernelShape::to_step_string`/`from_step_string`
      (roundtrip lewat file sementara, pola sama dengan `deep_clone`),
      `KernelShape::read_step` (baca file `.step` sungguhan), dan
      `write_step_compound` (gabung beberapa shape jadi SATU file STEP
      lewat `opencascade::primitives::Compound`, masing-masing tetap
      solid terpisah — bukan di-union). `KernelMesh::merge` ditambahkan
      sekalian (dipakai render viewport DAN export STL/OBJ multi-body —
      sebelumnya `cadraw-app::build_combined_body_mesh` menduplikasi
      logika gabung-mesh ini sendiri, sekarang keduanya pakai fungsi yang
      sama). 5 test baru (roundtrip STEP string, read_step, compound 2
      body, compound kosong error, merge menggeser indeks).
- [x] `cadraw-io` diisi pertama kali (sebelumnya kosong sejak Fase 0), 4
      modul:
      - `native`: format `.cadraw` — JSON pretty-printed (sengaja
        manusiawi-dibaca, bukan biner/kompresi, konsisten dengan
        `Profile`/`ProfileSegment` yang juga koordinat mentah bisa-baca).
        `CadrawFile { format_version, sketch, bodies }` — `format_version`
        (const `FORMAT_VERSION = 1`) ditolak `load` kalau file dibuat versi
        LEBIH BARU dari yang dikenal build ini (lebih aman daripada diam-
        diam salah baca; versi lebih lama tetap diterima, belum ada
        migrasi ditulis karena baru versi 1 yang pernah ada). Tiap
        `NativeBody` menyimpan `name`/`visible`/`step` (teks STEP lengkap
        lewat `to_step_string`) — SENGAJA tidak menyimpan `BodyId`: beda
        dari `EntityId` yang jadi rujukan silang constraint, tidak ada
        apa pun di file yang merujuk `BodyId` lintas body, jadi body
        cukup direkonstruksi sebagai daftar baru saat load.
      - `step_io`: export (1 body langsung, >1 body lewat
        `write_step_compound`) & import (`KernelShape::read_step`) file
        `.step` SUNGGUHAN di disk — beda dari `native` yang menyematkan
        teks STEP yang sama DI DALAM JSON `.cadraw`.
      - `mesh_export`: STL BINER (ditulis sendiri dari `KernelMesh`,
        bukan lewat `KernelShape::write_stl` milik kernel — supaya bisa
        menggabungkan banyak body jadi satu file; normal per-facet
        dihitung ulang dari cross product segitiga, bukan dipakai dari
        `mesh.normals` yang per-VERTEX) dan OBJ ASCII (satu blok
        `o <nama>` per body, indeks `v`/`vn` digeser per body). Sengaja
        HANYA export — STL/OBJ sudah berupa segitiga, tidak ada jalan
        balik ke B-rep, import diputuskan di luar lingkup Fase 5.
      - `dxf`: subset R12 ASCII minimal DITULIS SENDIRI (bukan crate
        `dxf` pihak ketiga) — LINE/CIRCLE/ARC saja, konsisten dengan
        filosofi proyek menulis sendiri lapisan tipis yang terkontrol
        penuh (solver LM, snap engine) alih-alih menarik dependensi besar
        untuk sebagian kecil kemampuannya. `Entity::Ellipse` dilewati saat
        export (dihitung, dilaporkan) — DXF R12 tidak punya entitas
        ELLIPSE. Import mem-parsing group-code minimal, entitas tak
        dikenal (TEXT/SPLINE/dst) dilewati & dihitung
        (`ImportResult::skipped`), bukan bikin seluruh import gagal.
      14 test baru (roundtrip native termasuk verifikasi `EntityId`
      persis sama setelah load, penolakan versi masa depan, penolakan
      JSON rusak, roundtrip STEP single & multi-body, header+jumlah
      segitiga STL, jumlah baris OBJ + offset indeks multi-body,
      roundtrip DXF Line/Circle/Arc dengan verifikasi sudut Arc, Ellipse
      dilaporkan skip, section ENTITIES hilang ditangani, entitas tak
      dikenal dilewati bukan gagal).
      **Bug ditemukan lewat test, bukan teori** (pola yang sama lagi
      dengan Fase 2/3): test `native`/`step_io` gagal ACAK dengan
      exception OCCT `InterfaceModel : AddWithRefs` — ternyata dua lock
      test TERPISAH (satu per modul) tidak cukup, karena `cargo test`
      menjalankan SEMUA modul dalam SATU binary test di banyak thread
      sekaligus, jadi test `native::*` dan `step_io::*` (dua-duanya
      menyentuh jalur transfer STEP OCCT yang sama) tetap bisa jalan
      BERSAMAAN lintas modul walau masing-masing sudah dikunci sendiri-
      sendiri. Diperbaiki dengan SATU lock `pub(crate)` di
      `cadraw-io::lib` dipakai bersama oleh `native`+`step_io` — beda
      dari `cadraw-kernel::tests::TEST_LOCK` yang cukup satu per crate
      karena krat itu cuma satu binary test tanpa modul lain yang ikut
      menyentuh OCCT.
- [x] `cadraw-app`: menu "📄 File" di toolbar (Baru, Buka…/Simpan/Simpan
      Sebagai… native, submenu Import STEP/DXF, submenu Export
      STEP/STL/OBJ/DXF) + dialog file native lewat `rfd`. Shortcut
      Ctrl/Cmd+O (Buka), +S (Simpan — jatuh ke Simpan Sebagai kalau
      dokumen belum pernah disimpan), +Shift+S (Simpan Sebagai, selalu
      dialog). `current_file_path: Option<PathBuf>` menentukan target
      "Simpan" langsung vs dialog. Export STEP/native SEMUA body (arsip
      dokumen penuh, terlepas visible); Export STL/OBJ cuma body
      `visible` (mewakili hasil cetak/tampilan fisik, konsisten dengan
      `build_combined_body_mesh` render viewport). Import STEP/DXF
      undo-able (`AddSolidCommand`/`InsertEntities` lewat undo stack yang
      sesuai — pola sama dengan Extrude/menggambar tool sketch); Baru/
      Buka SENGAJA mereset kedua undo stack (undo lintas-dokumen tidak
      masuk akal). Semua 10 aksi file juga ada di command palette lewat
      `PaletteAction::File(FileOp)` (satu variant menampung 10 `FileOp`,
      bukan 10 variant `PaletteAction` terpisah). Status hasil (sukses
      ATAU gagal — beda dari `model_status`/`constraint_status` yang
      cuma terisi saat gagal) tampil di status bar bawah.
      Seluruh workspace hijau: `build`/`clippy -D warnings`/`test` (72
      test: 3 kamera, 1 undo-core, 14 kernel, 36 sketch termasuk 18
      constraint & 4 model chain-builder, 14 cadraw-io); diverifikasi
      manual lewat smoke-run `cargo run -p cadraw-app` 6 detik tanpa
      panic.
- [ ] **Sengaja belum ada** (lingkup Fase 5 dipersempit ke inti yang bisa
      dikirim dalam satu putaran): import STL/OBJ (lossy, sudah segitiga,
      tidak ada jalan balik ke B-rep — keputusan sadar, bukan lupa),
      Ellipse di DXF (R12 tidak punya entitas itu — perlu upgrade ke
      subset R14+ kalau dibutuhkan nanti), spline/polyline/layer di DXF,
      memisahkan file STEP multi-solid jadi body terpisah saat import
      (baru satu `KernelShape` gabungan — butuh traversal topologi solid
      yang belum ada), autosave/recent-files/dirty-flag di title bar,
      drag-and-drop file ke jendela, resolusi konflik saat file dibuka
      lagi setelah berubah di disk.
- [ ] Verifikasi visual & UX (menu File, dialog native rfd, pesan status
      import/export) di device sungguhan — sama seperti fase-fase
      sebelumnya, belum bisa dicek dari sandbox agent (tidak ada akses
      WindowServer, dan dialog file native `rfd` butuh interaksi GUI
      sungguhan yang tidak bisa disimulasikan headless).

## Status Fase 6 — Port iPad (dikerjakan, satu blocker upstream)

- [x] **Spike cross-compile OCCT ke `aarch64-apple-ios` — risiko tertinggi
      sejak Fase 0, akhirnya dieksekusi di Fase 6**: `occt-sys` (dependensi
      `cadraw-kernel`) pakai crate `cmake` untuk build OCCT dari source.
      Ditemukan (via `cargo build --target aarch64-apple-ios`, bukan
      dugaan) crate `cmake` 0.1.58 SUDAH mengeset `CMAKE_SYSTEM_NAME=iOS`+
      `CMAKE_SYSTEM_PROCESSOR=arm64` otomatis saat cross-compiling, TAPI
      TIDAK PERNAH mengeset `CMAKE_OSX_SYSROOT`/`CMAKE_OSX_ARCHITECTURES`
      untuk target iOS (cabang itu di source cuma jalan untuk target yang
      mengandung `"darwin"`, bukan `"ios"`) — CMake diam-diam jatuh ke SDK
      macOS host, hasil kompilasi OCCT jadi object file bertanda platform
      macOS (`LC_BUILD_VERSION platform=1`, dicek langsung lewat
      `otool -l`), gagal link terhadap binary yang ditarget iOS
      (`ld: building for iOS, but linking in object file built for
      macOS`).
- [x] Diperbaiki **wiring**-nya (`crates/cadraw-kernel/ios/ios-toolchain.cmake`
      + env var `CMAKE_TOOLCHAIN_FILE_aarch64_apple_ios` di
      `.cargo/config.toml`, format nama yang dibaca crate `cmake` lewat
      `getenv_target_os`) — DIBUKTIKAN bekerja lewat probe CMake project
      berdiri sendiri (`cmake -DCMAKE_TOOLCHAIN_FILE=...` + `otool -l`
      hasil `.o` → `platform=2` alias iOS, bukan cuma baca cache).
- [ ] **TAPI toolchain file yang sama TIDAK cukup untuk occt-sys
      sungguhan** — dicoba 3 varian (nama SDK pendek `iphoneos`,
      `execute_process(xcrun...)` resolve PATH, PATH absolut di-hardcode
      + `CACHE ... FORCE`), ketiganya menghasilkan `CMakeCache.txt` OCCT
      sungguhan dengan `CMAKE_OSX_SYSROOT:STRING=` tetap KOSONG (dicek
      langsung tiap kali, bukan dugaan) walau `CMAKE_OSX_ARCHITECTURES`
      berhasil ter-set. Root cause paling mungkin (didukung bukti, bukan
      spekulasi murni): OCCT (source yang divendor `occt-sys`) TERNYATA
      punya jalur build iOS RESMI SENDIRI —
      `OCCT/adm/scripts/ios_build.sh` — yang meneruskan
      `-D CMAKE_OSX_SYSROOT:PATH=...` lewat ARGUMEN COMMAND-LINE cmake
      langsung, BUKAN lewat toolchain file generik. `occt-sys` 0.2.0
      punya `build.rs` yang HANYA memanggil `cmake::Config::new("OCCT")`
      generik (didesain untuk Android/Linux/Windows, tidak pernah
      diadaptasi untuk kebutuhan khusus iOS OCCT) — jadi ini **gap
      upstream di `occt-sys`/`opencascade-rs` 0.2.0**, bukan sesuatu yang
      bisa diperbaiki dari sisi CADRAW lewat env var/toolchain file saja.
      **Dihentikan setelah 4 percobaan rebuild penuh** (tiap percobaan
      ~20-30 menit) karena hasil identik tiap kali — melanjutkan tebak-
      tebakan toolchain lebih jauh tidak produktif, lihat "Langkah
      lanjutan" di bawah.
- [x] **Ditemukan & diperbaiki bug KEDUA yang independen** (ketemu selagi
      memverifikasi lewat `cargo check`, bukan `build`, untuk melewati
      blocker OCCT di atas): `eframe = { features = ["wgpu"] }` tanpa
      `default-features = false` tetap ikut mengaktifkan fitur DEFAULT
      eframe termasuk `"glow"` (backend OpenGL lewat `egui_glow`/
      `glutin`) walau CADRAW SELALU cuma pakai backend wgpu
      (`eframe::Renderer::Wgpu`, `cadraw-app/src/main.rs`). `glutin`
      TIDAK mendukung iOS — gagal compile (`match` non-exhaustive di
      `Surface<T>`, ~39 error). Diperbaiki: `eframe` di-set
      `default-features = false` + daftar ulang fitur default MINUS
      `"glow"` (lihat komentar di `Cargo.toml` root — `rwh_06` yang
      biasa lewat `"winit/default"` ternyata sudah diminta TANPA SYARAT
      oleh `[dependencies.winit]` eframe sendiri, jadi tidak hilang).
      Perilaku desktop tidak berubah (`cargo check --workspace` di macOS
      tetap hijau) — CADRAW memang tidak pernah memakai glow sama sekali.
- [x] **Files.app — putaran pertama nyata, bukan stub kosong**: `rfd`
      (dialog file native Fase 5) TERBUKTI TIDAK COMPILE SAMA SEKALI di
      iOS (tak ada backend UIKit, dibuktikan lewat probe crate terpisah)
      — digeser jadi target-specific dependency
      (`[target.'cfg(not(target_os = "ios"))'.dependencies]` di
      `crates/cadraw-app/Cargo.toml`). Pemanggilnya (8 titik di
      `main.rs` — Buka/Simpan/Import/Export STEP/STL/OBJ/DXF) dirapikan
      lewat 2 method baru `pick_open_path`/`pick_save_path` yang punya
      kembaran `cfg(target_os = "ios")`: BUKAN sekadar "belum didukung",
      tapi implementasi nyata berbasis folder `Documents` sandbox app
      (`ios_documents_dir`, dari env var `HOME` — pola standar iOS, tanpa
      dependensi bridging UIKit tambahan). "Simpan" menulis ke
      `Documents/<nama_default>` (mis. `untitled.cadraw`), "Buka"/Import
      mengambil file BERTANGGAL PALING BARU berekstensi cocok di folder
      itu. Folder ini muncul di Files.app ("Di iPad Ini ▸ CADRAW") HANYA
      kalau `Info.plist` app final menyematkan `UIFileSharingEnabled` +
      `LSSupportsOpeningDocumentsInPlace` — sudah didokumentasikan di
      `crates/cadraw-app/ios/Info.plist.template`. Batasan sadar:
      BUKAN `UIDocumentPickerViewController` sungguhan (tak ada dialog
      pilih file bebas) — itu butuh bridging UIKit (`objc2-ui-kit` atau
      sejenis), ditunda ke putaran berikutnya.
- [x] **Apple Pencil — diriset, bukan diasumsikan**: dibaca langsung
      source `winit` 0.30 (`event.rs`: `Touch.force: Option<Force>`,
      tersedia di iOS 9.0+) dan `egui-winit` 0.32
      (`on_touch`: `force: match touch.force { ... }` diteruskan apa
      adanya ke `egui::Event::Touch.force`). Kesimpulan: presisi pointer
      Apple Pencil (posisi, hover kasar via touch) OTOMATIS jalan lewat
      pipeline touch→pointer egui yang SUDAH ADA sejak Fase 0/1 — TIDAK
      ADA kode tambahan yang dibutuhkan untuk itu, dan data tekanan
      (`force`) SUDAH mengalir sampai ke `egui::Event::Touch` kalau
      kelak ada fitur yang butuh (mis. lebar garis sensitif-tekanan) —
      SENGAJA belum ditambahkan kode yang membaca `force` itu karena
      belum ada fitur nyata yang memakainya (CADRAW itu CAD presisi
      vektor, bukan app sketsa freehand — instrumentasi tanpa pemakai
      nyata cuma kode mati). Yang SENGAJA belum ada: gesture ganda-ketuk
      Pencil 2 (`UIPencilInteraction`) dan hover-sebelum-sentuh (kedua-
      duanya butuh bridging UIKit, di luar lingkup putaran ini).
- [x] `crates/cadraw-app/ios/Info.plist.template` — bukan dipakai
      otomatis (belum ada langkah yang mem-package binary jadi bundle
      `.app`), tapi referensi lengkap+beranotasi untuk langkah manual
      Xcode nanti: `CFBundleExecutable=cadraw`, orientasi landscape-utama
      (iPad tetap izinkan portrait), 2 key Files.app di atas.
- [x] **Ditemukan (bukan diasumsikan) bahwa tidak perlu shim Objective-C
      `main.m`/`AppDelegate` terpisah**: dibaca langsung source
      `winit` 0.30 (`platform_impl/ios/event_loop.rs`) —
      `EventLoop::run_app` di iOS memanggil `UIApplicationMain` SENDIRI
      dari proses yang sama, baca `argc`/`argv` proses lewat
      `_NSGetArgc`/`_NSGetArgv`. Artinya binary `[[bin]] name = "cadraw"`
      yang sudah ada (`crates/cadraw-app/Cargo.toml`) BISA LANGSUNG jadi
      executable `.app` iOS begitu OCCT beres — tidak perlu crate
      `staticlib` terpisah atau project Xcode dengan kode ObjC tambahan.
- [ ] **Langkah lanjutan untuk blocker OCCT** (belum dikerjakan, di luar
      lingkup putaran ini karena butuh pendekatan berbeda, bukan lagi
      sekadar env var): (a) patch `occt-sys`/`opencascade-rs`
      (fork+tempel lewat `[patch.crates-io]`) supaya `build.rs`-nya
      meniru `OCCT/adm/scripts/ios_build.sh` — meneruskan
      `CMAKE_OSX_SYSROOT` dkk. lewat argumen `-D` cmake langsung (bukan
      toolchain file) plus flag-flag lain yang dipakai skrip itu
      (`IPHONEOS_DEPLOYMENT_TARGET`, daftar modul aktif); ATAU (b) build
      OCCT untuk iOS SEKALI secara terpisah lewat `ios_build.sh` resmi di
      luar Cargo, lalu arahkan `occt-sys` ke hasilnya lewat env var
      `DEP_OCCT_ROOT` (disebutkan di dokumentasi `opencascade-sys`,
      belum dicoba). Keduanya pekerjaan baru yang cukup besar untuk sesi
      terpisah, bukan lanjutan kecil dari sesi ini.
- [ ] **Sengaja belum ada** (di luar 2 blocker/keputusan di atas): paket
      `.app` + code signing + provisioning profile + upload TestFlight —
      SEMUANYA butuh Xcode GUI interaktif + akun Apple Developer
      berbayar, TIDAK BISA dilakukan dari sandbox agent ini (tidak ada
      akses WindowServer/kredensial); project Xcode sungguhan belum
      dibuat (Info.plist masih template, bukan file aktif); testing di
      perangkat/simulator sungguhan (sama alasan — perlu Xcode GUI).
- [ ] Verifikasi lewat `cargo check --target aarch64-apple-ios`
      (type-check, BUKAN link — link final baru mungkin setelah blocker
      OCCT beres): `cadraw-render`+`cadraw-core`+`cadraw-sketch`+
      `cadraw-ui` bersih total. `cadraw-app` (seluruh app termasuk
      eframe/winit/wgpu/egui-winit DAN `cadraw-kernel`/`opencascade`)
      JUGA bersih total setelah 2 perbaikan di atas — `cargo check
      --workspace` di macOS host tetap hijau sepanjang perubahan ini
      (diverifikasi ulang, bukan diasumsikan aman).

## Status Fase 7 — Poles & Performa (dikerjakan, putaran pertama)

- [x] **Alat ukur**: `cadraw_sketch::measure` (murni fungsi baca-saja,
      TIDAK menyentuh `Sketch`/undo stack) — `distance` (jarak lurus) dan
      `angle_degrees` (sudut interior 0–180° di titik vertex,
      `atan2(det, dot)` supaya independen dari urutan klik titik). 6 test
      baru (segitiga 3-4-5, sudut siku-siku, sudut lurus 180°,
      order-independence, degenerate saat ray panjang nol).
      `cadraw-app`: 2 tool baru — **Ukur Jarak** (2 klik snap) dan **Ukur
      Sudut** (3 klik: awal/vertex/akhir), dikumpulkan di dropdown "📏 Ukur
      ▾" (pola sama dengan "Titik ▾") — non-destruktif, TIDAK masuk undo
      stack manapun. Hasil disimpan di `CadrawApp::measurements`,
      digambar permanen sebagai garis kuning
      (`cadraw_render::sketch::measurement_lines`, 3 test vertex-count)
      dan didaftar di jendela mengambang "📏 Pengukuran" (bisa dihapus
      satu-satu atau semua sekaligus, juga lewat command palette).
- [x] **Section View**: clip plane di render, BUKAN operasi kernel — sadar
      dipilih supaya bisa digeser real-time (tiap frame) tanpa memanggil
      OCCT sama sekali (beda dari Boolean yang benar-benar memotong
      B-rep). `shader.wgsl`/`SceneRenderer` dapat field `clip_plane:
      vec4<f32>` (`dot(normal, world) - offset`, fragment dengan hasil > 0
      di-`discard` di `fs_mesh`); nonaktif = normal nol vektor + offset
      1e9 (trik menghindari field "enabled" terpisah — selalu sangat
      negatif, tidak pernah memotong). `cadraw-app`: panel "✂ Section
      View" di panel Model 3D — checkbox aktif, sumbu X/Y/Z, slider offset
      (mm), "Balik arah potong" (membalik `(normal, offset)` SEKALIGUS,
      bukan cuma normal, supaya posisi potong di slider tidak ikut lompat
      saat cuma membalik sisi yang dibuang — bidang `dot(n,p)=d` dan
      `dot(-n,p)=-d` adalah bidang geometris yang sama persis).
- [x] **Temuan arsitektur nyata (dibuktikan lewat compile-time check,
      bukan dugaan)**: `KernelShape` — dan `opencascade::Shape` di
      baliknya — TERBUKTI TIDAK `Send` (`UniquePtr<TopoDS_Shape>` milik
      `cxx` tidak pernah diberi `unsafe impl Send` di binding
      `opencascade-rs` 0.2.0, konsisten dengan OCCT yang memang tidak
      thread-safe — akar masalah yang sama dengan bug `SIGABRT` STEP
      transfer di Fase 3). Ini membatasi arti "tessellation di thread
      terpisah" dari rencana awal: `KernelShape` TIDAK BISA dikirim ke
      thread lain sama sekali, jadi background thread cuma bisa dipakai
      untuk operasi yang bisa dibungkus lewat tipe `Send` murni
      (`PathBuf`/`String`/`KernelMesh`) di kedua ujungnya — bukan
      "jalankan operasi kernel apa saja secara paralel" (OCCT memang
      tidak mendukung itu). Latar belakang penuh untuk SEMUA operasi
      kernel (Extrude/Fillet/dst, bukan cuma Import) butuh rearsitektur
      command pipeline jadi async end-to-end — pekerjaan besar tersendiri,
      SENGAJA ditunda ke putaran lain, bukan dipaksakan setengah jalan di
      sini (sama semangat dengan blocker OCCT/iOS Fase 6: root-cause dulu,
      jangan tebak-tebak toolchain).
- [x] **`cadraw-kernel::KERNEL_LOCK`** (baru, PRODUKSI bukan cuma test):
      `Mutex<()>` global yang WAJIB dikunci di SETIAP fungsi publik kernel
      (14 titik: `tessellate`, `write_stl`, `write_step`, `read_step`,
      `to_step_string`, `from_step_string`, `extrude_profile`, `union`,
      `subtract`, `fillet_all`, `chamfer_all`, `shell_hollow`,
      `write_step_compound`, `make_filleted_box`) — dipegang HANYA di
      fungsi publik, bukan di helper privat (`deep_clone`/
      `tessellate_shape`) yang selalu dipanggil dari dalam fungsi publik
      yang sudah memegang lock, supaya tidak deadlock (`Mutex` std tidak
      reentrant). Sebelum Fase 7 ini tidak perlu — `cadraw-app` cuma
      pernah memanggil kernel dari UI thread tunggal; sekarang WAJIB
      karena `import_worker` menambah thread kedua yang bisa memanggil
      kernel. Menjamin tidak pernah ada 2 panggilan OCCT jalan bersamaan
      apa pun urutan klik user (mis. Extrude persis saat Import STEP
      latar belakang masih jalan) — cukup untuk KEBENARAN (tidak crash),
      BUKAN untuk paralelisme (OCCT tetap serial, itu memang batasannya).
      14 test `cadraw-kernel` tetap hijau, termasuk jalan multi-thread
      default (bukan cuma `--test-threads=1`).
- [x] **`cadraw-app::import_worker`** (baru): satu thread background
      berumur-panjang, job Import STEP diproses lewat `mpsc` channel.
      HANYA tipe `Send` murni yang lewat channel — `PathBuf` masuk,
      `(String teks STEP, KernelMesh)` keluar; `KernelShape` TIDAK PERNAH
      menyeberang thread (lihat temuan arsitektur di atas). Thread utama
      membangun `KernelShape` MILIKNYA SENDIRI dari string STEP lewat
      `from_step_string` untuk disimpan di `ModelDoc` — pola sama dengan
      "raw types at the kernel boundary" yang sudah dipakai
      `KernelMesh`/`Profile` sejak Fase 0/3. `import_step()` sekarang
      cuma `submit()` (non-blocking); `poll_import_worker()` (dipanggil
      tiap frame dari `update()`) memasang body baru begitu hasil siap,
      dan `ctx.request_repaint()` dipaksa selama ada job pending supaya
      hasil muncul secepat worker selesai, bukan menunggu event input
      (mouse bergerak dsb.) — egui default cuma redraw saat ada event.
      Import DXF SENGAJA tetap synchronous (murah, tidak menyentuh OCCT
      sama sekali, tidak butuh threading).
- [x] **Packaging**: `crates/cadraw-app/Cargo.toml` dapat
      `[package.metadata.bundle]` untuk `cargo-bundle` (nama, identifier,
      kategori, deskripsi) — metadata pasif, tidak mempengaruhi
      `cargo build`/`run`/`test` biasa sama sekali (diverifikasi: build
      tetap hijau setelah ditambahkan). `docs/PACKAGING.md` baru:
      langkah `cargo bundle --release` untuk `.app` macOS, catatan
      Windows/Linux (`cargo build --release` langsung jalan, belum ada
      installer/AppImage), dan daftar eksplisit di luar lingkup (code
      signing/notarization macOS, installer Windows, AppImage Linux,
      ikon `.icns` — semua butuh sertifikat berbayar/aset visual/GUI
      interaktif di luar sandbox agent, sama alasan dengan TestFlight iOS
      Fase 6). `cargo bundle --release` sendiri SENGAJA tidak dijalankan
      di sesi ini — release profile akan memicu build ulang OCCT dari
      nol (~8-40 menit, target dir profile terpisah dari debug), biaya
      besar cuma untuk verifikasi sintaks metadata; field yang dipakai
      (`name`/`identifier`/`icon`/`version`/`copyright`/`category`/
      `short_description`/`long_description`) sudah dicocokkan manual ke
      skema `cargo-bundle` yang terdokumentasi.
- [ ] **Sengaja belum ada** (lingkup Fase 7 putaran pertama dipersempit ke
      yang bisa diverifikasi tanpa rearsitektur besar atau build ulang
      OCCT berkali-kali, lihat catatan biaya sesi Fase 6): background
      thread untuk operasi kernel LAIN selain Import STEP (Extrude/
      Fillet/Chamfer/Boolean/Shell tetap synchronous di UI thread — lihat
      "Temuan arsitektur" di atas untuk kenapa ini pekerjaan besar
      tersendiri, bukan tambahan kecil); kontrol kualitas tessellation
      (`opencascade` 0.2.0 hardcode deflection 0.01 di `Mesher::new`,
      tidak ada API publik untuk mengubahnya tanpa memanggil
      `opencascade_sys::ffi` level rendah — di luar lingkup putaran ini);
      pengukuran 3D sungguhan (jarak/sudut antar titik body 3D, bukan
      cuma titik sketch 2D — butuh picking face/edge 3D yang belum ada,
      sama batasan dengan "Sengaja belum ada" Fase 3); label angka
      mengambang di titik 3D pengukuran (belum ada pipeline render teks
      di wgpu scene, hasil ditampilkan di panel "📏 Pengukuran" bukan
      di titik 3D-nya); code signing/notarization/installer (lihat
      `docs/PACKAGING.md`); actual run `cargo bundle` untuk memverifikasi
      `.app` hasil jadi.
- [ ] Verifikasi visual & UX (tool Ukur, panel Section View, notifikasi
      Import STEP latar belakang) di device sungguhan — sama seperti
      fase-fase sebelumnya, belum bisa dicek dari sandbox agent (tidak
      ada akses WindowServer). `cargo build`/`clippy -D warnings`/`test`
      seluruh workspace hijau (81 test: 4 kamera, 1 undo-core, 14 kernel,
      14 io, 6 render termasuk 3 measurement_lines baru, 42 sketch
      termasuk 6 measure baru), plus smoke-run `cargo run -p cadraw-app`
      tanpa panic startup.

## Status Fase 8 — Modeling 3D Lanjutan (dikerjakan, putaran pertama)

Riset API `opencascade-rs` 0.2.0 sebelum implementasi mengubah beberapa
dugaan lama di dokumen ini:

- **Revolve TERNYATA sudah ada** di binding (`Face::revolve`, 360° default)
  — Fase 3 dulu salah menyimpulkan ini belum tersedia (yang benar-benar
  tidak ada cuma sweep).
- **Sweep sepanjang jalur TIDAK ADA** sama sekali (tidak ada binding
  `BRepOffsetAPI_MakePipe`/`MakePipeShell` di `opencascade-sys`) — gap
  upstream sekelas blocker OCCT-iOS Fase 6, butuh menambal binding cxx
  sendiri. **Tetap didefer**, bukan dipaksakan.
- **Boolean intersect** ada level FFI (`BRepAlgoAPI_Common`, dipakai
  `AdHocShape::intersect`) tapi tidak diekspos di `Shape` publik seperti
  union/subtract — diimplementasi lewat `AdHocShape` sekali pakai.
- **Face-ray-cast SUDAH ADA** (`Shape::faces_along_ray`, exact level B-rep)
  — pas untuk face-picking. **Edge-ray-cast TIDAK ADA** — ditulis sendiri
  (closest-point ray-vs-segmen 3D, pola project yang sama dengan solver LM/
  snap engine/DXF writer: tulis lapisan tipis sendiri, bukan dependensi
  besar untuk sebagian kecil kemampuannya).

- [x] **`revolve_profile(profile, axis_origin, axis_dir, angle_degrees)`**
      (`cadraw-kernel`): `build_wire` (sudah ada) → `Face::from_wire` →
      `face.revolve(...)`. Validasi `axis_dir` non-degenerate SEBELUM
      panggil OCCT (dir nol → `Err`, bukan panic). `angle_degrees: None` =
      360° penuh (default binding); `Some(derajat)` didukung kernel tapi
      BELUM ada UI-nya (defer, lihat bawah). 2 test baru: revolve persegi
      yang TIDAK menyentuh axis → verifikasi geometris nyata (radius dalam
      >5, radius luar dalam rentang 15-25, tinggi dalam rentang y profil —
      bukan cuma "tidak panic"); axis degenerate → `Err`.
      `cadraw-app`: `ToolKind::Revolve` (shortcut **V**) — UX MENIRU PERSIS
      pola Mirror yang sudah ada: pilih profil dulu di tool Pilih (non-
      kosong), lalu 2 klik (snap aktif) jadi sumbu 2D di bidang XY. Hasil
      langsung `AddSolidCommand` (tipe existing, tidak perlu Command baru)
      kalau sukses.
- [x] **`build_wire_at_z(profile, z)`** (`cadraw-kernel`, refactor
      `build_wire` lama supaya terima parameter Z opsional — `build_wire`
      lama jadi wrapper tipis `build_wire_at_z(profile, 0.0)`) +
      **`loft_profiles(bottom, top, height)`** → `Solid::loft([bottom_wire,
      top_wire_at_z(height)])`. BUKAN loft lintas-workplane sungguhan
      (sketch CADRAW masih satu bidang XY, tidak ada konsep workplane sama
      sekali — dikonfirmasi lewat grep nol match) — profil ATAS cuma
      diangkat lewat translasi Z murni. 2 test: loft persegi→lingkaran
      verifikasi dasar tepat di Z=0 & puncak tepat di Z=height (sampling
      posisi vertex, bukan cuma triangle_count); tinggi nol → `Err`.
      `cadraw-app`: section baru di panel Model 3D (bukan tool viewport,
      panel-driven seperti Extrude) — tombol "Set Profil Bawah dari
      Seleksi" men-stage `pending_loft_bottom: Option<Profile>`, field
      Tinggi + tombol "Loft" pakai profil bawah ter-stage + seleksi sketch
      SAAT DIKLIK sebagai profil atas.
- [x] **`intersect(a, b)`** (`cadraw-kernel`) — `Shape` publik tidak
      expose `.intersect()` seperti union/subtract (cuma `AdHocShape`,
      wrapper tipis `BRepAlgoAPI_Common`) — `deep_clone` dulu (pola sama
      dengan fillet/chamfer, `a`/`b` asli pemanggil tidak tersentuh), lalu
      dibungkus `AdHocShape` sekali pakai. Hasil kosong (tidak
      bersinggungan) DIDETEKSI (`triangle_count() == 0` via helper privat
      `tessellate_shape`, BUKAN `KernelShape::tessellate()` publik — itu
      akan `lock_kernel()` lagi selagi guard `intersect` sendiri masih
      dipegang, `Mutex` std tidak reentrant, deadlock; ditemukan &
      diperbaiki sebelum sempat jadi bug produksi, bukan lewat teori) →
      `Err` rapi. 2 test: 2 box overlap → hasil lebih kecil dari union
      (bukti nyata "cuma irisan"); 2 box tidak overlap → `Err`.
      `cadraw-app`: `model::BooleanKind::Intersect` (varian baru,
      `BooleanCommand`/`try_new` existing dipakai apa adanya — tidak perlu
      Command baru) + tombol "Intersect" di baris Union/Subtract.
- [x] **Desain kunci: picking edge/face TANPA index yang rapuh lintas
      `deep_clone`.** `fillet_all`/`chamfer_all`/`shell_hollow` yang sudah
      ada semua memutasi lewat `deep_clone` (roundtrip STEP) — Face/Edge
      yang dipilih SEBELUM clone bukan sub-shape valid dari shape HASIL
      clone, dan index posisi di `shape.edges()`/`faces()` tidak terjamin
      stabil lintas roundtrip STEP (tidak pernah diverifikasi — jadi tidak
      boleh diasumsikan). **Solusi**: simpan **ray dunia** (`PickRay {
      origin, dir }`) yang dipakai klik, bukan index/handle. Saat apply,
      `deep_clone` dulu, lalu cast ULANG ray yang SAMA terhadap shape hasil
      clone (`faces_along_ray` untuk face; pencarian jarak-ray-ke-polyline
      custom untuk edge, TIDAK ADA primitif OCCT untuk ini) — karena
      `deep_clone` tidak memindah geometri di ruang dunia, ray yang sama
      selalu kena permukaan/tepi yang SAMA secara geometris. Menghindari
      SELURUH masalah index-stability/handle-identity lewat operasi
      geometris yang robust by construction, bukan asumsi yang butuh
      diverifikasi.
      **Divalidasi lewat test WAJIB** (dijalankan SEBELUM lanjut ke
      fillet/chamfer/shell per-tepi, pola root-cause-dulu yang sama dengan
      blocker OCCT/iOS Fase 6): `pick_face_consistent_across_deep_clone`/
      `pick_edge_consistent_across_deep_clone` — ray yang sama di-cast ke
      shape asli DAN ke hasil `deep_clone`-nya, titik hit HARUS sama dalam
      toleransi numerik kecil. Keduanya lulus — asumsi terbukti valid,
      bukan cuma masuk akal secara teori.
- [x] **`pick_face(shape, ray)`** / **`pick_edge(shape, ray, tolerance)`**
      (`cadraw-kernel`, publik — dipakai UI utk feedback interaktif) +
      helper privat `resolve_face_along_ray`/`resolve_edge_along_ray`
      (dipakai ULANG oleh fillet/chamfer/shell per-tepi di bawah, supaya
      tidak `lock_kernel()` dua kali dari dalam fungsi publik yang sama).
      `closest_point_ray_segment`: closest-point ray-vs-segmen 3D ditulis
      sendiri (pendekatan dua-langkah standar utk hit-testing interaktif,
      BUKAN solusi jarak-minimum-tersertifikasi — didokumentasikan sebagai
      batasan sadar). 1 test tambahan: ray meleset jauh dari shape →
      `pick_face` `None` (bukan salah nangkep sesuatu).
- [x] **`fillet_edges(shape, radius, rays, tolerance)`** /
      **`chamfer_edges(shape, distance, rays, tolerance)`** /
      **`shell_hollow_faces(shape, thickness, rays)`** (`cadraw-kernel`,
      fungsi TERPISAH dari `fillet_all`/`chamfer_all`/`shell_hollow` lama
      — perilaku lama TIDAK berubah sama sekali, backward-compatible
      penuh). `Shape::fillet_edges`/`chamfer_edges` TERNYATA sudah ada
      sebagai primitif publik di binding (`fillet_all` yang sudah ada
      sejak Fase 3 ternyata cuma `fillet_edges(radius, self.edges())` di
      baliknya) — tidak perlu menambal apa pun di binding.
      `Shape::hollow` TERNYATA SUDAH generic multi-face sejak awal
      (`shell_hollow` lama membatasi ke 1 face lewat `try_farthest`
      sendiri, bukan batasan binding). Semua 3 fungsi menolak `rays`
      kosong (error mengarahkan ke varian "semua tepi/arah otomatis").
      3 test geometris nyata: fillet 1 tepi spesifik pada box → jumlah
      vertex hasil LEBIH SEDIKIT dari `fillet_all` (bukti cuma 1 tepi yang
      kena, bukan semua 12); shell 2 face (top+bottom) → **jumlah FACE
      B-rep asli DAN jumlah triangle beda nyata dari shell 1-face** —
      ditemukan lewat test yang awalnya GAGAL (jumlah VERTEX tessellation
      kebetulan sama persis, 48==48, di box simetris ini) bahwa vertex
      count BUKAN proxy topologi yang reliabel untuk kasus ini, diperbaiki
      pakai face count + triangle count yang terbukti beda (10 vs 11 face,
      32 vs 28 triangle, dicek langsung); plus 2 test error (rays kosong,
      ray meleset dari shape mana pun).
      `cadraw-app`: `fillet_selected_body`/`chamfer_selected_body`/
      `shell_selected_body` (existing) dicabang — `selected_edges`/
      `selected_faces` tidak kosong → panggil varian `_edges`/`_faces`
      baru; kosong → perilaku LAMA (fungsi `_all`/`shell_hollow`) tidak
      berubah. `screen_to_plane_point` di-refactor: logika unprojection
      near/far diekstrak jadi `screen_to_ray` (dipakai bersama, perilaku
      sketch Z=0 TIDAK berubah). `handle_3d_picking` (baru) — diintersep
      di awal `handle_sketch_input` saat `PickMode` aktif (ortogonal
      terhadap `ToolKind`, dipicu tombol toggle "Pilih Tepi/Wajah Manual"
      di panel Model 3D, butuh PERSIS 1 body terpilih). `PickedEdge`
      menyimpan ray + polyline hasil pick (di-cache, highlight overlay
      garis oranye tidak query kernel ulang tiap frame render). Tombol
      "Reset Pilihan" mengosongkan seleksi picking.
- [ ] **Sengaja belum ada** (lingkup Fase 8 putaran pertama dipersempit ke
      yang bisa dibangun di atas API `opencascade-rs` 0.2.0 yang sudah ada
      + terverifikasi robust, sisanya bukan lupa): sweep sepanjang jalur
      (gap upstream `opencascade-sys`, butuh menambal binding cxx sendiri
      — pekerjaan besar terpisah, sekelas blocker OCCT-iOS Fase 6, bukan
      tambahan kecil); Revolve sudut PARSIAL (kernel sudah mendukung lewat
      `angle_degrees: Some(..)`, UI-nya belum — cuma 360° penuh); loft
      lintas-workplane sungguhan & sketch-on-face (butuh konsep workplane
      baru yang cross-cutting — menyentuh `screen_to_plane_point`, semua
      tempat DVec2→3D dipromosikan, DXF import/export — SETARA besarnya
      dengan async kernel pipeline yang didefer Fase 7, root-cause dulu
      sebelum dibangun di atas asumsi belum terverifikasi, bukan
      dipaksakan setengah jalan); picking BODY lewat klik viewport (baru
      face/edge pada body yang SUDAH terpilih dari daftar panel — klik
      viewport untuk GANTI seleksi body masih ditunda); toggle-off klik
      ulang tepi/wajah yang sama (v1 cuma menambah, ada tombol "Reset
      Pilihan" sebagai jalan keluar); highlight 3D wajah terpilih (baru
      hitungan angka di panel — butuh ekstraksi sub-mesh per-face yang
      tidak dibangun putaran ini); toleransi re-resolusi ray saat apply
      pakai konstanta tetap (`EDGE_REAPPLY_TOLERANCE_MM = 5.0`), bukan
      dihitung dari kamera (dipanggil dari tombol panel, bukan dari dalam
      `viewport()` yang punya akses `rect` layar — cukup karena ray/
      geometri tidak berubah antara pick dan apply, didokumentasikan di
      kode).
- [ ] Verifikasi visual & UX (tool Revolve, panel Loft/Intersect/picking
      edge-wajah, highlight tepi oranye) di device sungguhan — sama
      seperti fase-fase sebelumnya, belum bisa dicek dari sandbox agent
      (tidak ada akses WindowServer). `cargo build`/`clippy -D warnings`/
      `test` seluruh workspace hijau (96 test: 4 kamera, 1 undo-core, 29
      kernel termasuk 15 baru Fase 8, 14 io, 6 render, 42 sketch), plus
      smoke-run `cargo run -p cadraw-app` tanpa panic startup.

## Menjalankan

```bash
# App desktop (viewport 3D)
cargo run -p cadraw-app

# Smoke test kernel OCCT (setelah build pertama selesai)
cargo run -p cadraw-kernel --bin smoke

# Unit test
cargo test --workspace
```

Catatan build: `.cargo/config.toml` di root workspace mengatur
`CMAKE_POLICY_VERSION_MINIMUM=3.5` supaya `occt-sys` (dependensi
`cadraw-kernel`) tetap bisa dikonfigurasi CMake di mesin dengan CMake ≥
4.0 (CMakeLists.txt bawaan OCCT pakai `cmake_minimum_required` versi
lama). Build OCCT dari source pertama kali makan waktu beberapa menit,
setelahnya di-cache di `target/`.
