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
   Chamfer semua tepi, Shell/Hollow, render mesh 3D nyata; Revolve/sweep/
   sketch-on-face ditunda, lihat bawah]**
4. **UX shell** — toolbar kontekstual, command palette, radial menu iPad,
   target sentuh ≥44pt, tema. **[status: putaran pertama selesai — command
   palette, radial menu long-press, toggle tema, target sentuh global,
   lihat bawah]**
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
