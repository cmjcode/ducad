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
   Shell multi-face; extrude/push-pull per `SurfaceKind` (Plane/Cylinder/
   Cone/Sphere) + gizmo & HUD radial (`pull_dir`, "ΔR ± mm") ditambahkan
   lewat 4 sub-putaran lanjutan setelahnya (lihat "Status Fase 8 Lanjutan
   1–5" di bawah); sweep & sketch-on-face ditunda, lihat bawah]**

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
- [x] **Files.app & Apple Runtime (`objc2` & `block2`)**: `rfd`
      (dialog file native Fase 5) terisolasi untuk non-iOS target,
      sedangkan pada target Apple (`target_vendor = "apple"`), CADRAW kini
      menggunakan modul bridging modern `objc2` (0.6), `block2` (0.6),
      `objc2-foundation` (0.3), `objc2-app-kit` (macOS), dan `objc2-ui-kit`
      (iOS). Pemanggilan `pick_open_path`/`pick_save_path` di iOS memakai
      `apple::apple_documents_directory()` via `NSFileManager` Foundation
      untuk mengambil file bertanggal paling baru berekstensi cocok tanpa
      bergantung pada dialog desktop, terbukti lolos kompilasi bersih di
      `cargo check --target aarch64-apple-ios -p cadraw-app`. Folder ini muncul
      di Files.app ("Di iPad Ini ▸ CADRAW") ketika `Info.plist` app menyematkan
      `UIFileSharingEnabled` + `LSSupportsOpeningDocumentsInPlace`.
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
      dan didaftar di kartu "📏 Pengukuran" (bisa dihapus satu-satu atau
      semua sekaligus, juga lewat command palette) — awalnya jendela
      mengambang terpisah, **dipindah ke panel Properties kanan**
      (`FeatureInspector`, kartu selalu tampil non-selection-dependent
      seperti "Section View") supaya konsisten dengan lokasi panel tool
      lain; panel dipaksa tetap tampil (bypass auto-hide) selama tool
      Ukur/Ukur Sudut aktif ATAU masih ada hasil pengukuran tersimpan.
      Tool Ukur Jarak juga dapat dimension pill in-situ di tengah (median)
      garis selagi ditarik (klik pertama sudah, sebelum klik kedua
      commit) — pola sama persis dengan pill panjang tool Line di
      `dynamic_input_ui` (`ToolKind::Measure if pending_points.len() == 1`).
      Garis kuning-nya sendiri (baik masih preview ditarik MAUPUN sudah
      di-commit ke `CadrawApp::measurements`) sekarang bergaya dimension
      line CAD standar `↔`: kepala panah bentuk V di KEDUA ujung lewat
      `cadraw_render::sketch::measurement_arrowheads` (fungsi baru,
      terpisah dari `measurement_lines` supaya vertex tengah tool Ukur
      Sudut — 3 titik, 2 segmen — TIDAK ikut dapat panah, cuma titik
      pertama & terakhir; 4 test baru), dan nominal jaraknya ditaruh
      permanen di tengah garis (bukan cuma selagi ditarik) lewat pill
      baru di `dynamic_input_ui` yang membaca `Measurement::inline_value`
      (nilai pendek tanpa prefix "Jarak:"/"Sudut:", beda dari `label()`
      yang dipakai kartu panel Properties). Pill nominalnya lalu digeser
      dari offset di samping garis (gaya pill tool Line) supaya PAS DI
      ATAS garis kuningnya sendiri (posisi label = titik tengah garis
      langsung, tanpa offset normal). Tombol "📏 Ukur" di top bar
      (`TopBarEvent::ToggleMeasurements`) sekarang toggle sungguhan —
      klik lagi saat tool Ukur Jarak/Ukur Sudut sudah aktif balik ke
      Select (deactivate), bukan cuma reset ke Measure terus-menerus.
      Pill nominalnya sekarang juga DIPUTAR sejajar arah garisnya sendiri
      (bukan selalu horizontal) lewat `CanvasHud::render_dimension_pill_aligned`
      (fungsi baru di `cadraw-ui`: polygon konveks 4-sudut awalnya, lalu
      diganti `rounded_rect_local_points` — aproksimasi rounded-rect radius 10
      pakai 4 busur seperempat lingkaran per sudut, biar tetap membulat sama
      seperti `render_dimension_pill` yang tidak diputar — + `epaint::TextShape`
      yang punya field `angle`, keduanya diputar pakai `emath::Rot2` di sekitar
      titik tengah pill; fill dibikin agak transparan, alpha 245→210, supaya
      garis kuning di baliknya tetap samar kelihatan) supaya label tidak numpuk
      dengan garis pengukuran lain yang miring. Sudutnya dihitung dari selisih
      proyeksi layar kedua ujung garis (`CadrawApp::screen_line_angle`, helper
      baru) dan dinormalisasi ke -90°..90° supaya teksnya tidak pernah terbaca
      terbalik/kebalik walau garisnya miring ke kiri.
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

## Status Fase 8 Lanjutan — Direct Face Push/Pull (dikerjakan)

Di atas infrastruktur picking face berbasis ray-dunia (bagian di atas),
ditambahkan interaksi **push/pull langsung**: klik sisi (face) mana pun
dari solid apa pun (bukan cuma kubus — bekerja generik untuk hasil
Extrude/Revolve/Loft/Boolean apa saja, karena beroperasi di level B-rep
`Face`, bukan bentuk primitif tertentu) memunculkan gizmo panah 3D yang
bisa di-drag untuk menambah/memotong volume, meniru UX Shapr3D/Fusion.

- [x] **`extrude_face(shape, ray, distance)`** (`cadraw-kernel`) — cast
      ulang `ray` (pola `PickRay` yang sama dengan fillet/chamfer/shell
      per-tepi) ke `deep_clone` shape untuk resolusi `Face` yang valid,
      hitung normal keluar via `compute_face_normal_and_centroid`
      (Newell's method + flip berlawanan arah ray kamera), extrude face
      itu sepanjang `normal * distance` jadi volume sapuan. `distance >
      0` → `union` (nempel ke body asli); `distance < 0` → `subtract`
      (potong/pocket). 2 test geometris: extrude sisi atas & samping
      kubus, extrude tutup silinder — volume hasil diverifikasi lebih
      besar dari asli (bukti nyata union terjadi, bukan cuma "tidak
      panic").
      `pick_face_details(shape, ray)` (`FaceHit { hit_point, centroid,
      normal }`, publik) — dipakai UI utk gizmo interaktif; beda dari
      `pick_face` lama (cuma titik hit) yang tetap dipertahankan untuk
      pemanggil lain (Shell multi-face).
      `cadraw-app`: `active_face: Option<(BodyId, PickRay, FaceHit)>` —
      klik viewport di tool Pilih (Mode 3D) → `pick_body_face_at_cursor`
      ray-cast ke SEMUA body visible, ambil hit terdekat ke kamera.
      Gizmo panah digambar di `hit.centroid` sepanjang `hit.normal`
      (`build_overlay_lines`); drag diproyeksikan screen-space ke sumbu
      normal 3D lewat `project_screen_drag_to_world_axis` (generalisasi
      `project_screen_drag_to_extrude_axis` 2D lama, sekarang wrapper
      tipis di atasnya) → `face_gizmo_distance`; `drag_stopped()` commit
      lewat `extrude_active_face`. Panel kanan (`feature_inspector`)
      tampilkan card "Extrude Sisi (Face)" saat `active_face_selected`
      untuk input jarak presisi via keyboard.
- [x] **Bugfix: face push/pull tidak pernah muncul saat klik sisi solid
      di Mode 3D.** Root cause: handler klik tool Pilih menge-tes hit
      region sketsa 2D (proyeksi kursor ke `active_plane`) LEBIH DULU
      dari face-pick 3D. Sketsa dasar (mis. persegi sumber Extrude) tidak
      dihapus setelah di-extrude jadi solid — jadi proyeksi klik ke sisi
      mana pun kubus di Mode 3D hampir selalu jatuh di DALAM region
      sketsa dasar itu (masih ada, cuma tertanam di dalam solid & tak
      terlihat), region-hit selalu menang duluan, `pick_body_face_at_cursor`
      tidak pernah dipanggil. **Fix**: di Mode 3D (`!is_sketching`) dan
      klik tanpa Shift, coba `pick_body_face_at_cursor` PALING AWAL —
      kena solid → set `active_face` langsung (skip region/entity-hit
      sama sekali); meleset → fallback ke urutan lama (region → entity →
      face-pick lagi sebagai fallback terakhir), supaya extrude profil 2D
      dari Mode 3D tetap jalan. Ditambah eksklusi mutual eksplisit
      (`active_face = None` saat region/entity terpilih, `selected.clear()`
      saat face terpilih) — mencegah gizmo 2D dan gizmo face 3D aktif
      bersamaan. Mode Sketsa (`is_sketching == true`) dan klik+Shift tidak
      berubah perilakunya sama sekali.

## Status Fase 8 Lanjutan 2 — Deteksi Tipe Surface (dikerjakan, Fase 1)

Langkah pertama menuju klasifikasi face yang lebih kaya (dipakai fitur
mendatang seperti smart-boolean-cutting yang lebih presisi, atau hint UI
khusus per tipe permukaan) — sekadar deteksi/label tipe permukaan
geometris di balik sebuah `Face`, belum dipakai fitur apa pun.

- [x] **`Face::surface_kind() -> String`** (vendor `opencascade-0.2.0`,
      `src/primitives/face.rs`) — kembalikan nama kelas dinamis C++/OCCT
      dari permukaan di balik face (mis. `"Geom_Plane"`,
      `"Geom_CylindricalSurface"`, `"Geom_SphericalSurface"`). Disusun
      MURNI dari binding FFI yang sudah ada di `opencascade-sys` 0.2.0
      (`BRep_Tool_Surface` + `DynamicType` + `type_name`) — **tanpa
      menyentuh `opencascade-sys` sama sekali**, pola identik dengan patch
      `faces_along_ray_with_tolerance` yang sudah ada (lihat
      `vendor/README.md`).
      Fixture test tambahan: `AdHocShape::make_sphere(r)` (patch vendor
      baru juga, sama-sama tanpa sentuh `-sys` — cuma bungkus
      `BRepPrimAPI_MakeSphere_ctor` yang FFI-nya sudah ada tapi belum
      punya wrapper Rust publik).
- [x] **`SurfaceKind` enum** (`cadraw-kernel`) — `Plane | Cylinder | Cone |
      Sphere | Torus | Other`, `From<&str>` mem-parse keluaran
      `surface_kind()` (nama tak dikenal jatuh ke `Other`, bukan error —
      klasifikasi ini informasional). Field baru `FaceHit.surface_kind`
      diisi otomatis di `pick_face_details`. 3 test geometris: kubus → 6
      face semuanya `Plane`; silinder → 2 `Plane` (tutup) + 1 `Cylinder`
      (selimut); bola → 1 face `Sphere`.

## Status Fase 8 Lanjutan 3 — Vendor `opencascade-sys` + Binding Baru (dikerjakan, Fase 2)

Lanjutan langsung dari "Fase 8 Lanjutan 2" di atas: deteksi tipe surface
(Fase 1, `surface_kind()`) cukup dari FFI yang SUDAH ADA; giliran ini butuh
FFI yang BELUM ADA sama sekali (arah gizmo radial silinder/kerucut presisi
+ offset shell per-face) — dua kelas OCCT yang belum ada binding-nya di
`opencascade-sys` 0.2.0 upstream, jadi crate itu sendiri (bukan cuma
`opencascade`) sekarang juga di-vendor. Detail lengkap alasan & pola di
`vendor/README.md` (§ `opencascade-sys-0.2.0`) — ringkasan di sini:

- [x] **Vendor `opencascade-sys-0.2.0`** ke `vendor/`, terdaftar di
      `[patch.crates-io]` root `Cargo.toml` bersebelahan dengan
      `opencascade-0.2.0` yang sudah ada. `opencascade-0.2.0` vendored
      otomatis ikut memakai copy lokal ini (dependensinya
      `opencascade-sys = "0.2"` tak berubah, resolusi patch yang
      mengarahkan).
- [x] **Binding `BRepAdaptor_Surface`** (`include/wrapper.hxx` +
      `src/lib.rs`) — ctor dari `TopoDS_Face`, `GetType()` (enum
      `GeomAbs_SurfaceType`), dan accessor `gp_Cylinder`/`gp_Cone` (titik
      axis, arah axis, radius, semi-angle) — dasar utk arah gizmo radial
      & validasi tipe surface sebelum drag.
- [x] **Binding `BRepOffset_MakeOffset`** (offset shell per-face) —
      `Initialize(shape, offset, tol, mode, intersection, self_inter,
      join, thickening, remove_int_edges)`, `SetOffsetOnFace(face, d)`,
      `MakeOffsetShape()`, `IsDone()`, `Shape()`.
- [x] **Semua langkah yang bisa gagal secara geometris di OCCT
      dideklarasikan `Result<>`** di cxx bridge, dibungkus manual
      `try/catch(Standard_Failure) → throw std::runtime_error` di
      `wrapper.hxx` — `Standard_Failure` OCCT TERNYATA turunan
      `Standard_Transient`, BUKAN `std::exception` (diverifikasi dari
      header OCCT vendored), jadi mekanisme auto-catch cxx (yang cuma
      nangkep `std::exception`) tidak cukup; tanpa wrapper manual ini,
      exception OCCT tembus jadi `std::terminate` (abort proses), bukan
      `Result::Err`. Dibuktikan lewat 5 test baru,
      `vendor/opencascade-sys-0.2.0/tests/surface_and_offset.rs` (mis.
      `plane_face_reports_plane_type_and_rejects_cylinder_accessor` —
      manggil `Cylinder()` di face datar, harus `Err` rapi bukan crash
      test binary).
- [x] **Pola dobel-deklarasi enum ditemukan & didokumentasikan**: enum
      baru (`GeomAbs_SurfaceType`, `BRepOffset_Mode`, `GeomAbs_JoinType`)
      yang namanya bentrok dengan `enum` C++ asli OCCT (sudah
      di-`#include`) WAJIB juga dideklarasikan `type X;` di dalam blok
      `unsafe extern "C++"` cxx bridge — kalau tidak, cxx generate `enum
      class` sendiri dan gagal kompilasi C++ ("enumeration previously
      declared as unscoped"). Pola ini sudah lama dipakai diam-diam oleh
      `TopAbs_ShapeEnum`/`BOPAlgo_GlueEnum`/`IFSelect_ReturnStatus` di
      binding upstream — baru ketauan & didokumentasikan eksplisit pas
      nambah enum baru pertama kali sejak vendor dibuat.
- [x] 43 test `cadraw-kernel` + 5 test baru `opencascade-sys` lulus,
      seluruh workspace (`cargo test --workspace`) hijau.
- [ ] **Sengaja belum ada di putaran ini**: belum ada API level
      `cadraw-kernel`/`opencascade` yang memanggil `BRepAdaptor_Surface`/
      `BRepOffset_MakeOffset` baru ini — Fase 2 murni nambah FFI mentah di
      `opencascade-sys`, integrasi ke gizmo radial & fitur offset shell
      di level kernel/UI CADRAW ditunda ke putaran berikutnya.

## Status Fase 8 Lanjutan 4 — `extrude_face` Dispatch per Tipe Surface (dikerjakan, Fase 3)

Integrasi FFI mentah dari "Fase 8 Lanjutan 3" (`BRepOffset_MakeOffset`,
`BRepAdaptor_Surface`) ke level `cadraw-kernel`: `extrude_face` sekarang
dispatch berdasarkan `SurfaceKind` face yang di-pick, bukan selalu
extrude+boolean.

- [x] **`Shape::volume()` + `Shape::offset_on_face()`** (vendor
      `opencascade-0.2.0`, `src/primitives/shape.rs`) — `volume()` lewat
      `BRepGProp_VolumeProperties`/`Mass()` (FFI sudah ada, cuma belum ada
      wrapper publik); `offset_on_face(face, offset)` menyusun
      `BRepOffset_MakeOffset` (base offset `0.0` semua face,
      `SetOffsetOnFace` cuma di face terpilih, mode `BRepOffset_Skin` +
      `intersection=true` + join `GeomAbs_Intersection`) — TIDAK ada
      perubahan lagi di `opencascade-sys`, murni pemakaian FFI Fase 2 yang
      sebelumnya belum dipakai siapa pun. Varian error baru
      `Error::OffsetOnFaceFailed(String)` di `src/lib.rs` utk
      membungkus pesan `cxx::Exception`-nya. Detail lengkap di
      `vendor/README.md` (Perubahan #4, #6).
- [x] **`Face::cylinder_or_cone_radius()`** (vendor `opencascade-0.2.0`,
      `src/primitives/face.rs`) — coba `BRepAdaptor_Surface_cylinder`/
      `_cone` berantai, `None` kalau bukan keduanya (Sphere/Torus sengaja
      tidak tercakup — `opencascade-sys` belum ada binding
      `gp_Sphere`/`gp_Torus`, di luar cakupan Fase 2). Detail di
      `vendor/README.md` (Perubahan #5).
- [x] **`extrude_face` jadi dispatch per `SurfaceKind`** (`cadraw-kernel`):
      - `Plane` → jalur ASLI (extrude+union/subtract) **tidak disentuh
        sama sekali** — termasuk catatan deadlock `KERNEL_LOCK` yang sudah
        ada, tetap berlaku persis seperti sebelumnya.
      - `Cylinder`/`Cone`/`Sphere`/`Torus`/`Other` → jalur baru:
        `Shape::offset_on_face` langsung pada solid hasil `deep_clone`,
        `SetOffsetOnFace(face_terpilih, distance)`, hasilnya LANGSUNG
        solid baru (bukan dua langkah extrude+boolean seperti wajah
        datar — extrude wajah lengkung menghasilkan *swept surface* baru,
        bukan silinder/kerucut/bola dengan radius berbeda, jadi tidak
        cocok untuk union/subtract). Validasi batas SEBELUM memanggil
        OCCT: utk Cylinder/Cone, `radius + distance <= 0` → `bail!` jelas
        (pesan sebut radius saat ini & jarak drag); utk Sphere/Torus/Other
        (tidak ada pre-check radius presisi), `offset_on_face` sendiri
        `bail!` lewat `Result::Err`/`IsDone()==false` dari OCCT kalau
        geometri kolaps.
      - **Konvensi tanda tervalidasi empiris** (bukan asumsi): `distance`
        diteruskan APA ADANYA ke `SetOffsetOnFace` — utk wajah cembung
        (selimut luar silinder/bola), `distance > 0` membesarkan radius
        (drag keluar); utk wajah cekung (dinding lubang/interior),
        `distance > 0` justru MENGECILKAN radius (karena normal-keluar-
        dari-material OCCT di wajah itu mengarah ke sumbu) — perilaku
        OCCT standar, bukan bug, dibuktikan lewat test volume presisi di
        bawah.
- [x] **6 test baru volume** (`cadraw-kernel`, total sekarang 50 test,
      seluruh workspace `cargo test --workspace` hijau):
      - Silinder R=10,h=20 di-pull selimut +2 mm → volume persis
        `π·12²·20` (radius membesar).
      - Sama, push -3 mm → volume persis `π·7²·20` (radius mengecil).
      - Push -10 mm (radius jadi persis 0) → `bail!` jelas (pesan sebut
        "radius"), bukan gagal telat di OCCT.
      - Tabung berlubang (R_out=20, R_in=8), dinding lubang didorong
        radial ke dalam +2 mm → volume persis cocok dgn lubang mengecil
        ke R_in=6 (`π·(20²−6²)·20`).
      - Bola R=7 di-pull +1.5 mm → volume persis `4/3·π·8.5³` (offset satu
        face penuh = offset seragam seluruh bola, exact secara analitik).
      - Kerucut (fixture via `revolve_profile`, `BRepPrimAPI_MakeCone`
        sendiri tidak ada binding-nya — di luar cakupan Fase 2, sengaja
        tidak nambah FFI baru cuma utk test) — offset selimut kerucut
        TIDAK proporsional-sederhana ke volume (properti "cone sejajar":
        offset menggeser posisi puncak sepanjang sumbu), jadi test ini
        cuma validasi ARAH (tarik keluar = volume naik, tekan masuk =
        volume turun) + hasil tetap solid valid, bukan formula tertutup.
      - Regresi planar (tutup datar silinder) tetap pakai jalur lama,
        volume hasil extrude tetap tepat.
- [x] **Ditemukan & didokumentasikan saat menulis test**: `pick_face_details`
      TIDAK bisa dipakai utk verifikasi pick pada bola PENUH (1 face
      tertutup) — dia juga memanggil `compute_face_normal_and_centroid`
      (Newell's method di atas rantai boundary face), yang mengembalikan
      `None` utk boundary bola yang seam+2 pole degenerate (bukan loop
      tepi sederhana seperti kubus/silinder). Ini keterbatasan Newell's
      method yang SUDAH ADA sebelum Fase 3, tidak terkait dispatch offset
      — `extrude_face` jalur non-planar SENGAJA tidak butuh normal itu
      sama sekali jadi tidak kena masalah ini, tapi test yang butuh
      verifikasi tipe permukaan pada bola harus lewat resolver privat
      (`resolve_face_along_ray` + `Face::surface_kind()`) langsung,
      bukan `pick_face_details`. **Diperbaiki di Fase 4 di bawah.**

## Status Fase 8 Lanjutan 5 — Gizmo & UI per Tipe Surface (dikerjakan, Fase 4)

`FaceHit` (`cadraw-kernel`) sekarang membawa `pull_dir`: arah satuan yang
benar-benar dipakai gizmo push/pull, terpisah dari `normal` — untuk wajah
lengkung (Cylinder/Cone/Sphere), arah yang mengubah radius saat di-drag
BUKAN normal permukaan lokal (yang konstan per-face lewat Newell's method),
melainkan arah radial di titik hit itu sendiri.

- [x] **`FaceHit::pull_dir: (f64, f64, f64)`** (`cadraw-kernel`) — field
      baru di samping `normal`, dihitung `compute_pull_dir` per
      `SurfaceKind`:
      - `Plane` → identik `normal` (normal Newell, perilaku lama, TIDAK
        berubah).
      - `Cylinder`/`Cone` → radial di titik hit:
        `(hit_point − proyeksi hit_point ke garis sumbu).normalize()`.
        Proyeksi pakai axis params baru `Face::cylinder_or_cone_axis()`
        (lihat di bawah); fallback ke `normal` kalau axis tidak terbaca
        atau hit tepat di garis sumbu (radial nol).
      - `Sphere` → `(hit_point − centroid).normalize()`, `centroid` bola
        penuh (`Face::center_of_mass()`, GProp-based) SECARA MATEMATIS
        persis pusat bola (centroid luas permukaan bola simetris = pusat).
      - `Torus`/`Other` → fallback ke `normal` (belum ada rumus radial
        khusus utk tipe ini).
- [x] **`Face::cylinder_or_cone_axis()`** (vendor `opencascade-0.2.0`,
      `src/primitives/face.rs`) — method baru, `Option<(DVec3, DVec3)>`
      (titik acuan + arah satuan sumbu), pola identik
      `cylinder_or_cone_radius` (Fase 3): coba `BRepAdaptor_Surface_cylinder`/
      `_cone` berantai. Menyusun ulang binding `gp_Cylinder_location`/
      `_direction`/`gp_Cone_location`/`_direction` yang SUDAH ADA di
      `opencascade-sys` sejak Fase 2 (dulu cuma dipakai radius) — TIDAK ada
      perubahan lagi di `opencascade-sys` di fase ini. Detail di
      `vendor/README.md` (Perubahan #7).
- [x] **Bug ditemukan & diperbaiki sekaligus: `pick_face_details` kini
      berhasil utk bola PENUH.** Newell's method (`compute_face_normal_and_
      centroid`) tetap gagal `None` utk boundary bola (seam+2 pole
      degenerate, keterbatasan lama, tidak disentuh) — `pick_face_details`
      sekarang fallback ke `Face::center_of_mass()`/`Face::normal_at(hit)`
      (GProp-based, robust ke topologi apa pun) kalau Newell gagal, jadi
      bola bisa DIPILIH di viewport sama sekali (sebelumnya `None` tembus
      sampai UI — gap yang didokumentasikan di Fase 3 di atas). **Jebakan
      nyata yang ditemukan & diperbaiki saat implementasi**: fallback awal
      memproyeksikan `centroid` (pusat bola) ke permukaannya sendiri —
      kasus *degenerate* (berjarak sama ke SEMUA titik permukaan),
      `GeomAPI_ProjectPointOnSurf` gagal dapat solusi tunggal dan OCCT
      melempar `Standard_OutOfRange` yang TIDAK tertangkap cxx (abort
      proses via `std::terminate`, dibuktikan lewat crash nyata saat
      `cargo test -p cadraw-kernel`). Diperbaiki dgn memproyeksikan
      `hit_point` (titik yang TERBUKTI ada di permukaan, hasil ray-face
      intersection) — selalu well-defined.
- [x] **Gizmo (`cadraw-app`) pakai `pull_dir`, bukan `normal`** — 5 titik:
      hit-test gizmo (`check_near_gizmo`), kalkulasi kursor resize, direct
      drag handler layar penuh, render panah 3D visual, dan handle+pill
      HUD interaktif. Mekanik drag itu sendiri
      (`project_screen_drag_to_world_axis`, screen-space projection)
      **TIDAK berubah** — cuma sumbu arahnya sekarang bisa radial.
      `hit.normal` di `sketch_on_active_face` (orientasi bidang sketsa)
      SENGAJA tidak disentuh — bukan bagian mekanik push/pull.
- [x] **HUD dimension pill baru `format_face_gizmo_dimension_text`**
      (`cadraw-app`) — permukaan radial (`Cylinder`/`Cone`/`Sphere`) tampil
      `"ΔR +2.0 mm"` (prefix `ΔR` + tanda eksplisit, drag mengubah RADIUS);
      permukaan lain (`Plane`, dan fallback `Torus`/`Other`) tetap `"2.0
      mm"` polos, perilaku lama.
- [x] **4 test baru `pull_dir`** (`cadraw-kernel`, total sekarang 54 test,
      seluruh workspace `cargo test --workspace` hijau): `pull_dir` sama
      dgn `normal` di wajah datar; `pull_dir` radial persis (silinder &
      kerucut, titik hit di bidang simetri sehingga hasil bisa divalidasi
      exact, bukan cuma "condong ke arah X"); `pick_face_details` berhasil
      utk bola penuh dgn `pull_dir` radial dari pusat.
- [x] **Bug ditemukan & diperbaiki: precheck radius `extrude_face` salah
      arah utk dinding lubang (face CEKUNG/`Reversed`).** Precheck lama
      selalu memakai `current_radius + distance <= 0`, sama seperti wajah
      cembung (`Forward`) — tapi utk dinding lubang, normal-keluar-dari-
      material OCCT mengarah ke sumbu, jadi rumus radius barunya TERBALIK
      (`R − distance`, dibuktikan test `..._shrinks_hole_when_pushed_
      radially_inward` yg sudah ada: R=8 → 6 saat distance=+2). Akibat
      tanda salah: (1) memperbesar lubang lewat `distance` negatif —
      operasi VALID — ditolak keliru dgn pesan "radius ≤ 0"; (2)
      `distance` yang menutup lubang penuh (radius jadi 0) malah LOLOS
      precheck lalu gagal telat & generik di OCCT — persis yang ingin
      dicegah precheck ini. Diperbaiki dgn `Face::orientation()` (sudah
      ada di vendor, `face.rs:345`): `Forward` → `R + d`, `Reversed` →
      `R − d`. 2 test baru menyusul (total sekarang 56 test
      `cadraw-kernel`): lubang membesar MELEBIHI radius awal via distance
      negatif harus berhasil; lubang didorong persis sejauh radiusnya
      sendiri (menutup penuh) harus ditolak dgn pesan jelas soal radius.
- [x] **Bug ditemukan & diperbaiki: gizmo ter-anchor DI DALAM material utk
      face lengkung berukuran besar.** Sebelum perbaikan ini, ke-5 titik
      gizmo (lihat bullet "Gizmo pakai `pull_dir`" di atas) selalu anchor
      di `hit.centroid`. Utk selimut silinder/bola PENUH, `centroid`
      bukan titik di permukaan: Newell's method rata-rata dua lingkaran
      tepi → titik di SUMBU silinder; GProp `center_of_mass()` bola →
      PUSAT bola. Handle idle (`centroid + pull_dir·18`) pun ikut terkubur
      di dalam material utk radius > 18 mm — tak terlihat, tak bisa
      di-drag. (Sudah begitu sejak Fase 4 sebelumnya, tapi baru terasa
      sekarang karena face lengkung jadi fitur utama.) Diperbaiki dgn
      method baru **`FaceHit::gizmo_anchor()`** (`cadraw-kernel`):
      `Plane` → tetap `centroid` (perilaku lama, region datar biasanya
      kecil); selain `Plane` → `hit_point`, yang SELALU di permukaan
      (hasil ray-face intersection). Ke-5 titik gizmo di `cadraw-app`
      diganti dari `hit.centroid` mentah ke `hit.gizmo_anchor()`; mekanik
      `project_screen_drag_to_world_axis` TIDAK berubah. `hit.centroid` di
      `sketch_on_active_face` (orientasi bidang sketsa, bukan gizmo)
      SENGAJA tidak disentuh.
- [x] **Bug ditemukan & diperbaiki: `pull_dir` bola PARSIAL tidak radial
      dari pusat bola.** `compute_pull_dir` utk `Sphere` sebelumnya
      memakai `(hit − centroid).normalize()`. Utk bola PENUH itu benar
      krn `centroid` (GProp `center_of_mass()`, dipakai via fallback
      krn Newell SELALU gagal utk bola penuh) persis pusat bola. Tapi
      utk face bola PARSIAL (sudut hasil fillet bola, bola terpotong
      boolean) jalur Newell (`compute_face_normal_and_centroid`)
      BERHASIL, dan `centroid`-nya = rata-rata boundary loop FACE ITU —
      BUKAN pusat bola — jadi arah gizmo melenceng jauh (dibuktikan di
      test baru: sebelum perbaikan `pull_dir` di titik `(R,0,0)` jadi
      `(0.58, −0.58, −0.58)`, seharusnya `(1,0,0)`). Diperbaiki tanpa
      binding `gp_Sphere` baru: pakai `face.normal_at(hit)` langsung
      (normal permukaan bola SELALU radial dari pusat, valid utk
      penuh maupun parsial — properti geometris bola), dgn koreksi
      tanda thd `ray_dir` sama seperti fallback GProp di
      `pick_face_details`. Parameter `centroid` yg jadi tak terpakai di
      `compute_pull_dir` sekalian dibuang dari signature. 1 test baru
      `pull_dir_is_radial_on_partial_sphere_octant_face` (fixture:
      irisan bola dgn box oktan, 1/8 permukaan bola dibatasi 3 busur
      seperempat lingkaran — sengaja BUKAN full-sphere spy lewat jalur
      Newell, bukan fallback GProp) — total sekarang 57 test
      `cadraw-kernel`, `cargo test --workspace` hijau.

## Status Vertex Fillet Gizmo — Rounded Sudut 3D (dikerjakan bertahap)

Fitur baru: fillet SUDUT (vertex) 3D lewat gizmo, beda dari fillet TEPI
(edge) yang sudah ada sejak Fase 8 — user klik langsung di sudut body,
masukkan radius, semua tepi yang bertemu di sudut itu di-fillet sekaligus
jadi sudut membulat (spherical corner).

- [x] **Fase 1 — Kernel (`cadraw-kernel`).**
      `resolve_vertex_along_ray(shape, ray, tolerance) -> Option<DVec3>`:
      vertex bukan warga topologi tersendiri di query kernel (beda dari
      face/edge), jadi dikumpulkan sendiri dari endpoint SEMUA edge shape
      (banyak edge berbagi 1 vertex B-rep yang sama, di-dedup lewat
      epsilon 1e-6 mm), lalu dipilih yang jaraknya ke ray paling kecil dan
      masih dalam `tolerance`. **`pick_vertex(shape, ray, tolerance) ->
      Option<(f64,f64,f64)>`** — wrapper publik buat UI hover/klik.
      **`fillet_vertex(shape, radius, ray, tolerance) -> Result<KernelShape>`**
      — `ray` di-cast ULANG ke shape hasil `deep_clone` (pola sama dgn
      `fillet_edges`/`PickRay`), tepi-tepi yang endpoint-nya berimpit
      (epsilon SAMA, 1e-6 mm) dgn vertex terpilih dikumpulkan lalu
      di-fillet SEKALIGUS lewat `Shape::fillet_edges` — OCCT sendiri yang
      menghasilkan sudut membulat saat >1 tepi bertemu di 1 titik difillet
      bareng dgn radius sama. 5 test baru (`cadraw-kernel` sekarang 62
      test, `cargo test --workspace` — 142 test total — hijau).
- [x] **Fase 2 — State & Klik (`cadraw-app`).** State baru di `CadrawApp`:
      `active_vertex: Option<(BodyId, PickRay, (f64,f64,f64))>` (pola
      persis `active_face`, simpan `ray` bukan cuma titik supaya resolusi
      ULANG saat fillet sungguhan konsisten dgn body hasil `deep_clone`),
      `filleting_vertex_from_gizmo: bool`, `vertex_gizmo_radius: f64`
      (default 3.0), `vertex_gizmo_dimension_editing: bool`,
      `vertex_gizmo_edit_input: String`. Helper baru
      **`pick_body_vertex_at_cursor`** (pola sama `pick_body_face_at_cursor`,
      raycast semua body visible & ambil hit terdekat) pakai toleransi
      `pixel_tolerance_to_world * 12` (vs `* 14` buat face). Di handler
      klik mode 3D (`handle_sketch_input`): pick vertex dicoba DULUAN,
      SEBELUM `pick_body_face_at_cursor` — target vertex kecil secara
      visual dan gampang "ketutup" face di baliknya, jadi harus menang
      prioritas kalau kena keduanya. Hit → set `active_vertex` +
      `selected_bodies` + `model_status`, clear `active_face`. Klik di
      tempat lain (region 2D, face lain, entity sketch, klik kosong) atau
      Escape → `active_vertex` di-clear (Escape diberi prioritas tertinggi
      di chain-nya, sebelum clear pending-points/selected, supaya 1x
      Escape cukup buat batalkan gizmo vertex tanpa efek samping lain).
      `filleting_vertex_from_gizmo`/`vertex_gizmo_dimension_editing` masih
      belum dipakai di fase ini (warning dead-code wajar) — akan dipakai
      Fase 3 (render gizmo + HUD interaktif + eksekusi `fillet_vertex`).
- [x] **Fase 3 — Gizmo Visual & HUD (`cadraw-app`/`cadraw-render`).**
      Digabung 1 fase (render + interaksi + commit) alih-alih dipecah
      Fase 3/4 seperti draft awal di atas, sama kompleksitasnya dgn 1
      blok gizmo face yang sudah ada. **Overlay 3D**
      (`sketch_render::vertex_fillet_marker_lines`, `cadraw-render`):
      kotak kawat kecil TEPAT di vertex + garis putus-putus ke posisi
      handle + ikon kuadran lingkaran kecil (melambangkan "rounding") di
      dekat handle, semua di sepanjang `out_dir` — warna magenta
      `[1.0, 0.35, 0.85, 1.0]` sengaja beda dari cyan gizmo face
      (`FACE_GIZMO_COLOR`) supaya tidak tertukar visual saat sudut & sisi
      berdekatan di layar. Dipanggil dari `build_overlay_lines` lewat
      helper baru **`active_vertex_gizmo_dir(&self) -> Option<(Vec3, Vec3)>`**
      — posisi vertex + `out_dir = normalize(vertex − pusat AABB mesh
      body)` (AABB tessellasi, pola sama `pick_body_face_at_cursor`, BUKAN
      pusat solid B-rep presisi — cukup buat arah kasar "menjauhi body"),
      fallback `Vec3::Z` kalau vertex kebetulan di pusat AABB persis
      (arah nol). Dipakai bareng oleh overlay DAN HUD interaktif supaya
      keduanya selalu sepakat soal arah gizmo. **HUD interaktif**
      (`dynamic_input_ui`) — blok ke-4, cermin persis blok gizmo face:
      handle panah 2 sisi draggable (`CanvasHud::render_draggable_double_arrow_handle`)
      mengubah `vertex_gizmo_radius` via `project_screen_drag_to_world_axis`
      sepanjang `out_dir`, di-clamp `.max(0.1)` tiap update drag (jaga2
      kernel `fillet_vertex` menolak radius <= 0); pill teks "R
      <nilai><unit>" (`self.unit.format(...)`, BUKAN label "ΔR" spesial
      punya face gizmo radial — vertex fillet selalu radius polos); klik
      pill → toggle popup input teks; Enter/lost-focus → parse +
      `unit.to_internal_mm` + clamp `.max(0.1)` → commit. **Commit**
      (fungsi baru `commit_vertex_fillet`, dipanggil dari drag-stop DAN
      dari popup Enter/lost-focus): panggil `cadraw_kernel::fillet_vertex`
      dgn `Self::EDGE_REAPPLY_TOLERANCE_MM` (sama dgn `fillet_edges`,
      bukan toleransi piksel — tidak ada akses `rect` layar di titik
      commit ini) — sukses → `ReplaceGeometryCommand` lewat
      `self.model_undo.execute` (jalur undo SAMA dgn `fillet_selected_body`),
      `active_vertex` di-clear; gagal (mis. radius kebesaran, OCCT
      menolak) → `model_status` pesan error, shape body ASLI tidak
      tersentuh (`fillet_vertex` bekerja di atas `deep_clone`). Tidak ada
      test baru (murni UI/rendering — sama alasan dgn catatan "Fase
      3"/"Fase 4" akhir modeling 3D lain di dokumen ini) — `cargo test
      --workspace` tetap 142 test hijau (62 di `cadraw-kernel`, tidak
      berubah dari Fase 1).

- [x] **Fase 4 — Perbaikan UX picking sudut ("klik sudut kubus meleset").**
      Bug report: 5 klik berturut-turut di "sudut kubus" konsisten jatuh di
      pixel yang sama, ray tembus tepat di RUSUK vertikal pojok box (mis.
      `(70, -39.98, 48)`), 22.5mm dari vertex B-rep terdekat — jauh di atas
      toleransi lama. Akar masalah: user secara visual mengklik rusuk
      tegak pojok (seperti sudut tembok), bukan titik vertex kecilnya;
      `pick_vertex` bekerja benar sesuai desain, tapi (a) toleransinya
      malah lebih ketat dari face padahal komentarnya bilang "lebih
      longgar", (b) tidak ada fallback ke edge, (c) vertex sama sekali
      tidak digambar jadi tidak ada feedback visual sebelum klik. 3
      perbaikan, tidak ada perubahan skema data tersimpan:
      1. **Toleransi vertex 12px → 18px** (`pick_body_vertex_at_cursor`,
         `cadraw-app`) — dulu malah lebih ketat dari face/edge (14px),
         padahal komentarnya sendiri bilang "lebih longgar"; sekarang
         benar-benar melebihi keduanya sesuai maksud aslinya.
      2. **Edge fillet gizmo baru** — kernel sudah punya `pick_edge`
         (dipakai `PickMode::Edge` manual sejak Fase 8), sekarang dipakai
         juga sbg fallback klik OTOMATIS: di `handle_sketch_input`,
         urutan coba jadi vertex → **edge (baru)** → face. Helper baru
         **`pick_body_edge_at_cursor`** (cermin `pick_body_face_at_cursor`,
         toleransi 14px). State baru `active_edge: Option<(BodyId,
         PickRay, (f64,f64,f64))>` (titik KLIK pada rusuk, dipakai
         langsung sbg jangkar gizmo — beda dari `active_vertex` yang
         menyimpan titik vertex resmi) + `filleting_edge_from_gizmo`,
         `edge_gizmo_radius` (default 3.0), `edge_gizmo_dimension_editing`,
         `edge_gizmo_edit_input`. Gizmo visual & HUD **identik** dgn gizmo
         vertex fillet (`active_edge_gizmo_dir` cermin
         `active_vertex_gizmo_dir`, overlay pakai
         `vertex_fillet_marker_lines` warna magenta yang sama, blok ke-5 di
         `dynamic_input_ui` cermin blok ke-4) — cuma commit-nya fungsi baru
         **`commit_edge_fillet_single`**: `cadraw_kernel::fillet_edges`
         dgn 1 ray (bukan `fillet_vertex`). Semua tempat yang meng-clear
         `active_face`/`active_vertex` (klik region 2D, klik entity sketch,
         klik kosong, Escape) diperbarui ikut clear `active_edge`.
      3. **Marker vertex terlihat + hover highlight** — fungsi baru
         **`shape_vertices(shape) -> Vec<(f64,f64,f64)>`** di
         `cadraw-kernel` (endpoint semua edge, dedup sama seperti dipakai
         `pick_vertex`/`fillet_vertex`, diekstrak jadi helper privat
         `collect_vertices` supaya logikanya cuma 1 tempat) + fungsi
         render baru **`sketch_render::vertex_dot_markers`** (silang 3
         sumbu kecil per vertex, abu-abu redup normal / kuning besar saat
         di-hover). `build_overlay_lines` menggambar marker ini di SEMUA
         vertex tiap body visible saat mode 3D (`!is_sketching`). State
         baru `hovered_vertex_marker: Option<(BodyId, (f64,f64,f64))>`
         dihitung ULANG tiap frame kursor ada di viewport
         (`handle_sketch_input`, lewat `pick_body_vertex_at_cursor` yang
         sama dipakai klik) — dilewati selagi drag gizmo mana pun biar
         tidak query kernel percuma tiap frame drag. Tidak ada test baru
         (murni UX/rendering, sama alasan dgn fase gizmo lain) —
         `cargo test -p cadraw-kernel` tetap 62 test hijau.

### Rounding Parametrik (lanjutan gizmo rounding — dua arah & bisa dibatalkan)

Perbaikan atas dua keluhan gizmo rounding vertex/edge: (1) drag ke dalam
tidak bisa (radius di-clamp min 0.1, commit destruktif — sekali bulat tak
bisa siku lagi), (2) klik sudut yang SUDAH bulat jatuh ke face-pick →
gizmo extrude → `offset_on_face` malah MEMBESARKAN objek.

- `cadraw-kernel`: fungsi publik baru `clone_shape` (deep-clone snapshot
  B-rep utk app; test `clone_shape_independent_of_original`). 63 test.
- `cadraw-app`: rounding jadi PARAMETRIK — `RoundHistory` per body
  (`round_history: HashMap<BodyId, RoundHistory>`) menyimpan shape DASAR
  sebelum rounding pertama + daftar `RoundFeature` (kind Vertex/Edge, ray,
  anchor, radius, polyline rusuk). Commit (`commit_round`) menyusun daftar
  fitur baru lalu REBUILD dari dasar (`build_rounded_shape`, ray di-resolve
  ulang berurutan), bukan memfillet hasil fillet. Radius < `ROUND_SHARP_MM`
  (0.2) = fitur dihapus → sudut kembali menyiku. Klik pada sudut/rusuk yang
  sudah bulat diintersep `find_round_feature_near` (jarak hit-point face ke
  anchor fitur ATAU polyline rusuk asli ≤ radius·1.5 + toleransi layar) →
  buka kembali gizmo dgn radius fitur (`editing_round`), pill menampilkan
  "R 0 (siku)" di bawah ambang. Drag handler kedua gizmo kini clamp 0.0
  (bukan 0.1) dan TIDAK me-reset nilai saat commit gagal. Riwayat
  di-invalidate (`round_history.remove`) begitu body diubah operasi lain:
  Cut Extrude, boolean (kedua sumber), Fillet/Chamfer panel, Shell,
  Extrude/Cut Face, Delete body — karena base-nya tidak lagi
  merepresentasikan shape berjalan. Riwayat hidup per sesi (tidak
  diserialisasi); undo (`ReplaceGeometryCommand`) tetap jalan, edit
  sesudah undo rebuild deterministik dari base yang sama.

## Status Ruler Properties — Checkbox "Tampilkan Semua Ukuran" (dikerjakan)

Permintaan user: di kartu "📏 Pengukuran" (ruler properties, panel
Properties kanan), tambah checkbox yang kalau aktif melabeli nominal
ukuran SEMUA elemen di kanvas — bukan cuma hasil tool "Ukur" eksplisit —
supaya user langsung tahu ukuran tiap elemen tanpa harus mengukur satu-satu.
Kanvas CADRAW tidak punya toggle mode 2D/3D terpisah (sketsa bidang aktif +
body 3D selalu ditumpuk render bersamaan), jadi satu checkbox menangani
kedua konteks ("mode sketch 2D atau mode 3D") sekaligus:

- `cadraw-kernel`: fungsi publik baru `edge_dimensions(shape) ->
  Vec<EdgeDimension>` (`EdgeDimension = ((f64,f64,f64), f64)`, titik
  tengah arc-length + panjang). Traversal `shape.edges()` yang sama dengan
  `resolve_edge_along_ray`/`collect_vertices`, TAPI di-dedup pakai epsilon
  jarak endpoint (seperti `collect_vertices`) — `shape.edges()` memuat tiap
  rusuk 2x (sekali per wajah yang memakainya) untuk padatan tertutup, tanpa
  dedup box sederhana melapor 24 rusuk alih-alih 12. Test
  `edge_dimensions_reports_all_box_edges` (extrude rect 30×20 tinggi 15,
  assert 12 rusuk + multiset panjang `{15×4, 20×4, 30×4}`). 64 test kernel.
- `cadraw-app::model`: `BodyGeometry` dapat field `edge_dims`, dihitung
  SEKALI saat geometri body dibuat/berubah (constructor
  `from_shape`/`from_shape_with_mesh`, pola sama dengan cache `mesh`) —
  bukan tiap frame render, supaya toggle checkbox tidak memicu traversal
  OCCT berulang. `from_shape_with_mesh` juga dipakai jalur Import STEP
  (`poll_import_worker`) yang sebelumnya membangun `BodyGeometry` lewat
  struct literal langsung (bypass `from_shape`) — sekarang tetap dapat
  `edge_dims` walau mesh-nya sudah dihitung worker di thread lain.
- `cadraw-ui::feature_inspector`: `FeatureInspectorState.show_all_dimensions`
  + `InspectorEvent::ToggleShowAllDimensions`. Kartu Pengukuran diubah dari
  kondisional (dulu cuma tampil kalau tool Ukur aktif/ada hasil) jadi SELALU
  render, supaya checkbox-nya selalu bisa ditemukan.
- `cadraw-app::main`: `CadrawApp::show_all_dimensions`, disinkronkan ke/dari
  inspector state seperti `auto_hide_properties`. Method baru
  `render_all_element_dimensions` dipanggil dari `dynamic_input_ui` saat
  checkbox aktif:
  - 2D: iterasi `self.sketch().entities` bidang aktif — Line dapat pill
    sejajar garis (`render_dimension_pill_aligned`, pola sama dgn pill
    pengukuran/preview tool Line), Circle/Arc dapat pill "R …", Ellipse
    dapat pill "Rx … Ry …".
  - 3D: iterasi `self.model.geometry`, lewati body yang `!visible` (pola
    sama dgn marker vertex), label tiap `edge_dims` lewat
    `world_to_screen_pos` + `render_dimension_pill` non-aligned (radius
    scope 3D dikonfirmasi ke user: SEMUA body visible, bukan cuma body
    terseleksi).
  - Tidak ada perubahan pada tool "Ukur" eksplisit (`self.measurements`) —
    checkbox ini independen, murni tambahan label pasif.

`cargo build --workspace` bersih, `cargo test --workspace -- --test-threads=1`
148 test hijau (64 cadraw-kernel, 45 cadraw-sketch, sisanya cadraw-core/
cadraw-io/cadraw-render/cadraw-ui).

**Perbaikan susulan — nominal dobel di rusuk dasar body:** user melaporkan
banyak label dobel, "terutama bagian bawah, dimana sumber sketch menjadi
3d". Sebab: body hasil Extrude/Loft/Revolve dibangun DARI profil sketsa
bidang aktif, dan entitas profilnya TETAP ada di `self.sketch()` sesudahnya
(biar bisa diedit ulang) — jadi rusuk-rusuk pada wajah body yang berimpit
dengan bidang sketsa itu dilabeli DUA KALI: sekali oleh loop 2D (pill
sejajar garis, `render_dimension_pill_aligned`), sekali lagi oleh loop
rusuk 3D (pill datar, `render_dimension_pill`) — di posisi dunia yang
identik. Untuk rusuk yang sejajar layar (mendekati horizontal), sudut pill
2D kebetulan ≈ 0° sehingga kedua pill bertumpuk sempurna dan TERLIHAT cuma
satu; untuk rusuk yang miring di layar, pill 2D (miring) dan pill 3D
(selalu datar) terlihat jelas terpisah jadi dua label — persis pola yang
dilaporkan user (label kelihatan dobel di sisi miring, kelihatan tunggal
di sisi datar, padahal sebenarnya dobel di semua sisi yang berimpit
sketsa).

Perbaikan: `render_all_element_dimensions` merekam `line_anchors_2d`
(posisi dunia + panjang tiap Line 2D yang sudah dilabeli) selagi loop 2D
jalan, lalu loop rusuk 3D melewati (skip) rusuk mana pun yang posisi
tengah + panjangnya cocok (toleransi 1e-3 mm posisi, 1e-3 mm panjang)
dengan salah satu anchor itu — jadi tiap rusuk cuma dapat SATU label,
diprioritaskan yang dari sketsa 2D (lebih presisi, sejajar garisnya).
`cargo test --workspace -- --test-threads=1` tetap 148 test hijau (fix
murni logika render, tidak menyentuh kernel/model).

**Perbaikan susulan 2 — label rusuk 3D disejajarkan ke garisnya:** user
minta nominal ukuran SELALU sejajar arah garis/rusuknya masing-masing,
tetap sejajar walau kamera diputar — sebelumnya label rusuk 3D memakai
`render_dimension_pill` (selalu datar horizontal), beda dari label sketsa
2D yang sudah pakai `render_dimension_pill_aligned` (ikut arah garis).
Perlu titik AWAL+AKHIR tiap rusuk (bukan cuma titik tengah) supaya sudut
layarnya bisa dihitung dari proyeksi kamera SEKARANG, sama seperti
`screen_line_angle` yang dipakai sketsa 2D:

- `cadraw-kernel::EdgeDimension` diperluas dari `(mid, length)` jadi
  `(mid, start, end, length)` — `edge_dimensions` mengembalikan endpoint
  dunia tiap rusuk juga, bukan cuma titik tengah arc-length-nya. Test
  `edge_dimensions_reports_all_box_edges` diperluas: validasi korda
  start↔end sama dengan `length` (rusuk box selalu lurus) dan `mid` persis
  di tengah start↔end. 64 test kernel tetap hijau.
- `cadraw-app::main`: `screen_line_angle` (2D) di-refactor, bagian inti
  "proyeksi 2 titik dunia → sudut layar dinormalisasi -90°..90°" dipecah
  jadi `screen_angle_between_world_points` yang menerima `Vec3` dunia
  langsung — dipakai ULANG oleh loop rusuk 3D di
  `render_all_element_dimensions` (ganti `render_dimension_pill` jadi
  `render_dimension_pill_aligned`, sudut dihitung dari `start`/`end` rusuk
  TIAP FRAME, bukan sekali saat body dibuat) — jadi label rusuk 3D ikut
  berputar sejajar rusuknya persis seperti label sketsa 2D, real-time
  mengikuti orbit kamera. `cargo test --workspace -- --test-threads=1`
  tetap 148 test hijau.

## Status Perbaikan — Live Preview 3D saat Drag Gizmo Face (dikerjakan)

User melaporkan: drag gizmo extrude PERTAMA KALI (dari sketch 2D ke solid)
sudah menampilkan animasi perubahan 3D real-time selagi ditarik, tapi drag
gizmo pada FACE 3D yang sudah ada (klik sisi/face lalu tarik) TIDAK — solid
baru berubah bentuk saat drag dilepas. Diminta keduanya real-time, plus
nominal jarak baru langsung muncul.

Root cause: `build_combined_body_mesh` (dipanggil tiap frame untuk
membangun mesh viewport) punya blok "Live Extrude / Boolean Cut preview"
yang HANYA dipicu oleh `self.extruding_from_gizmo` (flag gizmo sketch) —
memanggil ulang `extrude_profile_active_plane` + `.tessellate()` tiap
frame selagi drag. Tidak ada blok setara untuk
`self.extruding_face_from_gizmo` (flag gizmo face 3D) — geometri final
cuma dihitung sekali lewat `extrude_active_face` saat `drag_stopped()`.
Label jarak (`face_gizmo_distance` → pill dimensi) sebenarnya SUDAH
ter-update tiap frame drag — cuma terasa tidak real-time karena mesh-nya
diam.

Perbaikan (`cadraw-app::main`, `build_combined_body_mesh`):

- Body target disembunyikan (skip render mesh asli) selagi
  `extruding_face_from_gizmo` aktif DAN body itu = target dari
  `self.active_face` — pola sama dengan skip body target pada boolean cut
  gizmo sketch (`gizmo_is_cutting`/`gizmo_target_body`), supaya preview
  tidak dobel-render dgn mesh lama.
- Blok baru "4. Live Face Extrude preview": selagi
  `extruding_face_from_gizmo` dan `face_gizmo_distance` bukan nol,
  panggil ulang `cadraw_kernel::extrude_face(&target_geo.shape, ray,
  face_gizmo_distance)` + `.tessellate()` TIAP FRAME (cermin persis blok
  extrude sketch), lalu masukkan mesh hasilnya ke buffer viewport
  (warna cyan, sama dgn highlight face terpilih). Gagal (mis. radius
  akan jadi ≤ 0 saat drag ekstrem) dibiarkan lewat — frame itu saja
  tanpa preview, sama seperti pola blok sketch yang sudah ada.

Hasil: drag gizmo face 3D sekarang menampilkan perubahan solid real-time
persis seperti gizmo sketch — dan karena mesh sudah ikut real-time, pill
jarak yang sebelumnya sudah live sekarang terasa konsisten dgn perubahan
bentuknya. `cargo build -p cadraw-app` bersih, `cargo test -p
cadraw-kernel` tetap 64 test hijau (tidak menyentuh logika `extrude_face`
itu sendiri, cuma memanggilnya lebih sering).

**Perbaikan susulan — gizmo rounding (vertex/edge fillet di pojokan) juga
belum real-time:** user melaporkan drag gizmo rounding di sudut/rusuk
("bikin rounded") punya masalah PERSIS sama — bentuk bulatnya baru muncul
saat drag dilepas. Root cause identik: `build_combined_body_mesh` tidak
punya blok live-preview utk `filleting_vertex_from_gizmo`/
`filleting_edge_from_gizmo`, geometri final cuma dihitung sekali lewat
`commit_round` (dipanggil dari `commit_vertex_fillet`/
`commit_edge_fillet_single`) saat `drag_stopped()`.

Rounding parametrik (lihat "Status Vertex Fillet Gizmo") lebih rumit dari
extrude — tiap body punya `round_history` (shape dasar + daftar fitur
rounding berurutan) dan `commit_round` menyusun ulang daftar fitur
(tambah baru ATAU edit radius fitur yang sedang di-`editing_round`) lalu
me-rebuild shape dari dasar via `build_rounded_shape`. Live preview-nya
perlu logika SAMA tapi dry-run:

- Method baru `round_gizmo_preview_shape(&self, kind: RoundKind) ->
  Option<(BodyId, KernelShape)>` — cermin persis bagian penyusunan fitur
  di `commit_round` (klon daftar fitur dari `round_history`, ganti radius
  fitur yg sedang diedit ATAU push fitur baru dgn radius SAAT INI), lalu
  `build_rounded_shape(base, &features)`, TAPI tidak menyentuh
  `round_history`/`model_undo`/`model_status` — murni menghitung shape
  utk digambar. `None` kalau gizmo tidak aktif, radius masih di bawah
  ambang "siku" (`ROUND_SHARP_MM`), atau rebuild OCCT gagal (radius
  kebesaran dsb, frame itu saja tanpa preview).
- `build_combined_body_mesh`: body target disembunyikan (skip mesh asli)
  selagi `filleting_vertex_from_gizmo`/`filleting_edge_from_gizmo` aktif
  DAN radius sudah lolos `ROUND_SHARP_MM` (supaya tidak "hilang tanpa
  pengganti" saat radius masih dianggap siku) — pola sama dgn skip body
  target extrude face. Blok baru "5. Live Rounding preview": panggil
  `round_gizmo_preview_shape` TIAP FRAME utk kind Vertex dan Edge,
  tessellate hasilnya, masukkan ke buffer viewport (abu-abu CAD normal,
  bukan cyan — rounding bukan highlight seleksi).

`cargo build -p cadraw-app` bersih, `cargo test -p cadraw-kernel` tetap
64 test hijau (logika `fillet_vertex`/`fillet_edges` tidak diubah, cuma
dipanggil lebih sering lewat jalur dry-run baru).

## Status Perbaikan — Crash Gizmo Rounding di Batas Ujung Objek (dikerjakan)

User melaporkan: drag gizmo rounding (fillet vertex/edge) sampai batas
ujung objek (radius terlalu besar utk tepi/sudut yang dipilih) membuat
SELURUH aplikasi langsung close, log:
```
libc++abi: terminating due to uncaught exception of type StdFail_NotDone
zsh: abort      cargo run -p cadraw-app
```

Root cause: `BRepFilletAPI_MakeFillet`/`BRepFilletAPI_MakeChamfer::Shape()`
(OCCT) melempar `StdFail_NotDone` kalau build fillet/chamfer gagal
(`IsDone()==false`) — dan `StdFail_NotDone` adalah turunan
`Standard_Failure`, BUKAN `std::exception` (pola yang sudah didokumentasikan
di `vendor/README.md` sejak Fase 2 utk `BRepAdaptor_Surface`/
`BRepOffset_MakeOffset`). Mekanisme cxx yang otomatis menerjemahkan
exception C++ jadi `Result::Err` cuma nangkep `std::exception`, jadi
binding `opencascade-0.2.0::Shape::fillet_edges`/`chamfer_edges` yang
manggil `.Shape()` mentah TANPA try/catch bikin exception-nya TEMBUS lewat
cxx dan memicu `std::terminate` — abort seluruh proses `cargo run`, bukan
error Rust yang bisa ditangani `commit_round`/`round_gizmo_preview_shape`
di `cadraw-app`. Live preview real-time yang baru ditambahkan (lihat
"Perbaikan susulan" di atas) membuat crash ini jauh lebih gampang kepancing
— radius ekstrem kini dievaluasi TIAP FRAME selagi drag, bukan cuma sekali
saat dilepas.

Perbaikan (vendor patch, ikut pola `Standard_Failure`→`Result` yang sudah
ada — lihat `vendor/README.md` § `opencascade-0.2.0` Perubahan #8 &
§ `opencascade-sys-0.2.0` Perubahan #3 utk detail lengkap):

- `vendor/opencascade-sys-0.2.0`: dua binding baru,
  `BRepFilletAPI_MakeFillet_shape_checked`/`BRepFilletAPI_MakeChamfer_
  shape_checked` — `Shape()` dibungkus try/catch(Standard_Failure) +
  rethrow `std::runtime_error` di `wrapper.hxx` (helper
  `rethrow_standard_failure_as_runtime_error` yang sudah ada, bukan baru),
  dideklarasikan `Result<&TopoDS_Shape>` di cxx bridge. ADDITIVE murni —
  binding `Shape()` mentah lama TETAP ADA (dipakai `Solid::fillet_edge`/
  `AdHocShape::fillet_edges`/`chamfer_edges` yang tidak dipakai
  cadraw-kernel).
- `vendor/opencascade-0.2.0`: `Error::FilletFailed(String)` (variant baru,
  pola sama `OffsetOnFaceFailed`); `Shape::fillet_edge`/`fillet_edges`/
  `chamfer_edge`/`chamfer_edges`/`fillet`/`chamfer` sekarang balikin
  `Result<(), Error>` (dulu `()`), manggil versi `_checked` di atas;
  `BooleanShape::fillet_new_edges`/`chamfer_new_edges` ikut disesuaikan
  (tidak dipakai cadraw-kernel, cuma biar konsisten & tidak diam-diam
  mengabaikan `Result`).
- `cadraw-kernel`: `fillet_edges`/`fillet_vertex`/`chamfer_edges`/
  `fillet_all`/`chamfer_all`/`make_filleted_box` menyalurkan `Result` baru
  itu lewat `.context("...")?` — pesan error jelas ("radius fillet
  terlalu besar untuk tepi/sudut terpilih") alih-alih crash. Signature
  publik kernel (`Result<KernelShape>`) TIDAK berubah, jadi TIDAK ADA
  perubahan di `cadraw-app` — `commit_round` dan `round_gizmo_preview_shape`
  (lihat "Perbaikan susulan" di atas) sudah menangani `Err` dgn benar
  (pesan status / skip frame preview), sekarang cuma benar-benar
  menerimanya lewat jalur normal, bukan lewat proses yang sudah mati.

Regresi dibuktikan 3 test baru di `cadraw-kernel`
(`fillet_edges_oversized_radius_errors_not_crashes`,
`fillet_vertex_oversized_radius_errors_not_crashes`,
`chamfer_edges_oversized_distance_errors_not_crashes`) — radius/jarak
1000mm pada box 30×20×15: tanpa patch SIGABRT test binary, dengan patch
`Result::Err` biasa. `cargo build --workspace` bersih, `cargo test
--workspace -- --test-threads=1` 151 test hijau (67 cadraw-kernel, +3 dari
sebelumnya).

**Perbaikan susulan — radius "sukses" tapi melebihi batas wajar (bukan
crash):** user kirim screenshot — SETELAH fix crash di atas, drag gizmo
rounding sampai "batas ujung objek" masih tidak berhenti: sudut box
"memakan" seluruh sisi jadi bentuk baji/quarter-cylinder, bukan sudut
membulat wajar. Beda root cause dari fix sebelumnya — OCCT sendiri masih
`IsDone()==true` di radius segini (bukan `StdFail_NotDone`), jadi
`Shape::fillet_edges`/`fillet_vertex` TIDAK `Err`; CADRAW sendiri yang
belum punya batas atas geometris wajar utk radius/jarak rounding.

- `cadraw-kernel`: fungsi privat baru `max_fillet_radius(shape,
  target_edges) -> f64` — batas radius/jarak = SETENGAH panjang tepi
  TERPENDEK yang terlibat (tepi target sendiri + semua tepi lain di shape
  yang endpoint-nya berimpit dgn endpoint tepi target, epsilon sama dgn
  `fillet_vertex`/`collect_vertices`). Dipanggil sbg precheck SEBELUM
  manggil OCCT (pola sama validasi radius silinder/kerucut di
  `extrude_face`) di `fillet_edges`/`fillet_vertex`/`chamfer_edges`/
  `fillet_all`/`chamfer_all` — `bail!` dgn pesan jelas ("radius fillet
  (X mm) melebihi batas geometris ... (maks Y mm)") kalau radius/jarak
  melebihinya.
- Dua fungsi publik baru, `max_vertex_fillet_radius`/
  `max_edge_fillet_radius(shape, ray, tolerance) -> Option<f64>` — versi
  query murni (tanpa mutasi/fillet beneran) dari batas yang sama, dipakai
  `cadraw-app` MENGUNCI nilai radius gizmo SELAGI DRAG, bukan cuma
  menolak SETELAH radius terlanjur jauh melebihi batas. Tanpa ini, nilai
  internal gizmo (`vertex_gizmo_radius`/`edge_gizmo_radius`) terus
  bertambah tanpa batas selagi mouse bergerak walau preview 3D-nya sudah
  berhenti berubah (`fillet_vertex` mulai `Err`, live preview freeze) —
  begitu drag dilepas, commit GAGAL TOTAL (sudut balik siku, radius yang
  dikirim sudah kadung di atas batas) alih-alih berhenti/menetap di
  radius maksimum yang valid, persis kebalikan dari yang diminta user.
- `cadraw-app::main`: kedua drag handler gizmo rounding (vertex & edge,
  blok "4"/"5" di HUD draw) sekarang meng-clamp
  `self.vertex_gizmo_radius`/`self.edge_gizmo_radius` ke hasil
  `max_vertex_fillet_radius`/`max_edge_fillet_radius` TIAP FRAME drag —
  dihitung terhadap `round_history[body].base` kalau ini EDIT fitur
  rounding yang sudah ada (sudut sudah jadi permukaan blend membulat di
  `geo.shape`, `resolve_vertex_along_ray` tidak akan menemukan vertex
  tajam di situ lagi — base yang masih siku), atau `geo.shape` langsung
  kalau rounding pertama kali (pola sama dgn `build_rounded_shape` yang
  selalu rebuild dari `base`, bukan shape final).

2 test baru di `cadraw-kernel`
(`fillet_vertex_radius_exceeding_shortest_edge_errors_before_reaching_occt`,
`fillet_edges_radius_exceeding_shortest_touching_edge_errors_before_reaching_occt`,
lihat "Perbaikan susulan 2" di bawah utk nama final setelah dikoreksi)
— radius 10mm pada box 30×20×15 (tepi terpendek 15mm): SENGAJA jauh lebih
kecil dari radius 1000mm di test crash sebelumnya (yang sudah bikin OCCT
sendiri gagal) supaya benar-benar menguji precheck geometris BARU, bukan
cuma re-test fix crash lama. `cargo build --workspace` bersih, `cargo
test --workspace -- --test-threads=1` 153 test hijau (69 cadraw-kernel,
+2 dari sebelumnya).

**Perbaikan susulan 2 — batas geometris kelewat konservatif (baru
separuh jalan):** user kirim screenshot LAGI — sekarang gizmo rounding
berhenti terlalu CEPAT, baru sekitar separuh menuju ujung tepi objek,
padahal seharusnya bisa melengkung sampai mendekati ujung tepinya. Bug
di fix sebelumnya sendiri: `max_fillet_radius` salah pakai `min_len /
2.0` sebagai batas.

Root cause geometris yang benar: utk sudut ORTOGONAL (semua sudut
box/prism, target utama gizmo rounding CADRAW), titik singgung fillet di
sepanjang tepi tetangga berjarak PERSIS `radius` dari titik sudut
(`tan(45°) == 1` — sudut antar 2 wajah tegak lurus dibagi dua = 45°),
BUKAN `radius/2`. Jadi radius maksimum yang aman kira-kira SAMA DENGAN
panjang tepi tetangga terpendek itu sendiri (titik singgungnya baru
menyentuh UJUNG satu tepi tetangga di radius maksimum, bukan setengahnya
di tengah tepi).

Perbaikan (`cadraw-kernel::max_fillet_radius`): buang `/ 2.0`, balikin
`min_len` langsung. Kalau radius sampai PERSIS menyentuh sudut tetangga
(kasus batas paling ekstrem, marjinal secara numerik), `fillet_edges`/
`fillet_vertex` masih bisa `Err` dari OCCT sendiri — itu SUDAH aman
(tidak crash, fix `StdFail_NotDone` di "Perbaikan" sebelumnya), jadi
tidak perlu margin pengaman tambahan di precheck ini.

2 test lama (`*_exceeding_half_shortest_*`, radius 10mm vs batas lama
7.5mm) DIGANTI 4 test baru — radius 10mm sekarang valid (< batas baru
15mm) jadi tidak lagi cocok jadi kasus uji "harus ditolak":
- `fillet_vertex_radius_near_full_shortest_edge_succeeds` /
  `fillet_edges_radius_near_full_shortest_touching_edge_succeeds` —
  radius 14mm (dekat batas baru 15mm) HARUS sukses — bukti formula
  `/2.0` tidak lagi aktif (kalau regresi, 14 > cap lama 7.5 → gagal).
- `fillet_vertex_radius_exceeding_shortest_edge_errors_before_reaching_occt`
  / `fillet_edges_radius_exceeding_shortest_touching_edge_errors_before_reaching_occt`
  — radius 20mm (> batas baru 15mm) tetap ditolak.

`cargo build --workspace` bersih, `cargo test --workspace --
--test-threads=1` 155 test hijau (71 cadraw-kernel, +2 neto dari
sebelumnya — 2 test lama diganti nama+radius, 2 test baru ditambah).

**Perbaikan susulan 3 — objek 3D hilang total saat drag mencapai/
melewati batas radius:** user kirim screenshot ketiga — saat drag gizmo
rounding mencapai/melewati batas geometris, objek 3D-nya HILANG TOTAL
(bukan cuma berhenti membesar) sampai drag dilepas, baru muncul lagi
dengan radius maksimum begitu dilepas.

Root cause di `build_combined_body_mesh`: pola "sembunyikan body asli,
gambar preview sebagai gantinya" (blok 1 vs blok 3/4/5) memutuskan
SEMBUNYIKAN body berdasarkan FLAG drag aktif semata (mis.
`filleting_vertex_from_gizmo`), BUKAN berdasarkan apakah preview
penggantinya benar-benar berhasil dihitung frame itu. Begitu radius
mendekati batas paling ekstrem, `round_gizmo_preview_shape` (via
`fillet_vertex`) bisa `Err` dari OCCT sendiri WALAU sudah lolos precheck
`max_fillet_radius` kita (radius PERSIS di batas = kasus paling marjinal
secara numerik OCCT, sudah diantisipasi aman dari SISI CRASH sejak
"Perbaikan" pertama, tapi belum dari sisi RENDER) — body sudah kadung
disembunyikan tapi preview penggantinya `None`, jadi viewport kosong.

Perbaikan (`cadraw-app::main::build_combined_body_mesh`, restrukturisasi
menyeluruh): SEMUA preview live (boolean cut dari sketch, extrude face,
rounding vertex/edge) di-precompute LEBIH DULU sebagai `Option<(BodyId,
KernelMesh)>` (atau `Option<KernelMesh>` utk extrude sketch non-cut yang
tidak menyembunyikan body apa pun) SEBELUM loop body normal — loop body
normal lalu menyembunyikan body HANYA kalau preview penggantinya
terbukti `Some` utk body itu; bagian "gambar preview" di bawahnya tinggal
memakai nilai yang sama, bukan menghitung ulang. Pola ini berlaku SERAGAM
utk keempat jenis preview (bukan cuma rounding) supaya kelas bug yang
sama tidak muncul lagi di gizmo extrude face/sketch cut (yang sebenarnya
punya celah identik, cuma belum sempat dilaporkan).

Efek samping positif: kondisi eksplisit "radius harus lolos
`ROUND_SHARP_MM` dulu baru body disembunyikan" (ditambahkan di
"Perbaikan susulan" pertama) jadi tidak perlu lagi — sudah otomatis benar
lewat cek `Some`, karena `round_gizmo_preview_shape` sendiri sudah
`return None` di bawah `ROUND_SHARP_MM` (satu sumber kebenaran, bukan dua
syarat terpisah yang bisa tidak sinkron).

`cargo build --workspace` bersih, `cargo test --workspace --
--test-threads=1` tetap 155 test hijau (murni refactor render, tidak
menyentuh kernel).

**Perbaikan susulan 4 — formula geometris TERBUKTI dua kali salah,
diganti validasi trial langsung ke OCCT:** user kirim screenshot keempat
— objek 3D sudah tidak hilang, tapi radius masih berhenti SEBELUM
benar-benar mencapai pinggir objek (R 61.89mm ditampilkan, tapi kurva
belum sampai ujung). Ini bug KETIGA di area yang sama: formula precheck
`/2.0` (kelewat konservatif #1), lalu formula precheck tanpa `/2.0`
(kelewat konservatif #2, dari `min_len` "tepi tersentuh" yang scope-nya
salah). Pola berulang ini adalah SINYAL bahwa masalahnya bukan "rumus
mana yang benar" — masalahnya adalah PENDEKATAN "coba tebak formula
geometris" itu sendiri: blend fillet 3 arah di 1 vertex (atau bahkan
fillet 1 tepi) bukan geometri sederhana yang bisa diringkas 1 rumus
tertutup akurat untuk segala kasus; OCCT-lah yang tahu persis batasnya.

Perbaikan (perombakan pendekatan, bukan sekadar formula baru):

- `cadraw-kernel`: precheck manual `max_fillet_radius` DIHAPUS TOTAL
  (fungsi `max_fillet_radius`/`max_vertex_fillet_radius`/
  `max_edge_fillet_radius` semua dihapus) dari `fillet_edges`/
  `fillet_vertex`/`chamfer_edges`/`fillet_all`/`chamfer_all` — sumber
  kebenaran SEKARANG murni `Shape::fillet_edges`/`chamfer_edges`
  (`IsDone()`/`Err` dari OCCT langsung, SUDAH aman dari crash sejak fix
  `StdFail_NotDone`). Alasan lengkap didokumentasikan di doc-comment
  `fillet_vertex`.
- `cadraw-app::round_gizmo_preview_shape`: signature berubah, `radius`
  jadi PARAMETER eksplisit (bukan dibaca langsung dari
  `self.vertex_gizmo_radius`/`edge_gizmo_radius`) — supaya bisa dipanggil
  dgn radius KANDIDAT (belum tentu diterima) maupun radius TERKINI
  (dipakai render), fungsi yang SAMA.
- Drag handler gizmo vertex & edge fillet: alih-alih menghitung
  `new_radius` lalu meng-clamp dgn formula, sekarang menghitung
  `candidate_radius` dari delta mouse lalu MENCOBA LANGSUNG
  `round_gizmo_preview_shape(kind, candidate_radius)` (yang ujungnya
  memanggil `fillet_vertex`/`fillet_edges` OCCT sungguhan) — radius
  cuma DITERIMA (`self.vertex_gizmo_radius = candidate_radius`) kalau
  trial itu SUKSES. Kalau gagal, radius tetap di nilai valid TERAKHIR.
  Efeknya: `self.vertex_gizmo_radius`/`edge_gizmo_radius` SELALU berupa
  radius yang PROVEN bisa dibangun OCCT (bukan perkiraan) — pill dan
  visual render otomatis sinkron (render pass memanggil fungsi yang sama
  dgn radius yang sama, dijamin sukses), DAN commit saat drag dilepas
  otomatis sukses (radius yg dikirim sudah kadung ter-validasi).
- Trade-off yang disadari: drag TERUS-MENERUS memanggil operasi OCCT
  penuh (deep_clone + fillet build) — SEKALI utk validasi kandidat di
  drag handler, SEKALI LAGI utk mesh preview di render pass — 2x lipat
  dari sebelumnya. Diterima utk model sederhana (target CADRAW saat ini);
  kalau nanti terasa lag di model kompleks, optimisasi lanjutan adalah
  meng-cache hasil trial drag handler dan dipakai ulang oleh render pass
  (BUKAN precheck formula lagi).

2 test lama yg menguji AMBANG formula (`fillet_*_radius_near_full_*`,
`fillet_*_radius_exceeding_*`) diganti 2 test baru
(`fillet_vertex_radius_near_shortest_edge_succeeds_without_manual_precheck`,
`fillet_edges_radius_near_shortest_touching_edge_succeeds_without_manual_precheck`)
yang HANYA membuktikan radius mendekati tepi tetap sukses TANPA precheck
manual (tidak menghardcode ambang mana pun milik CADRAW) — test
"exceeding" dihapus karena tidak ada lagi ambang CADRAW utk diuji;
batasnya sekarang murni keputusan OCCT.

`cargo build --workspace` bersih, `cargo test --workspace --
--test-threads=1` 153 test hijau (69 cadraw-kernel, neto -2 dari
sebelumnya — 4 test lama dihapus, 2 test baru ditambah).

**Perbaikan susulan 5 — klik sudut LAIN utk rounding baru malah tidak
memunculkan gizmo:** user melaporkan — setelah sukses rounding sudut
pertama sampai radius besar (dekat batas objek, sesuai "Perbaikan
susulan 4"), klik sudut LAIN sama sekali tidak memunculkan gizmo apa pun.
Root cause ketemu lewat log debug yang user tempel: klik jatuh di
`cabang ROUND_EDIT` (mengedit fitur rounding #0 yang SUDAH ADA), padahal
titik klik ada di wajah DATAR (`normal=(0,1,0)`) jauh dari lengkungan
sudut pertama.

`find_round_feature_near` (dipanggil sebelum vertex/edge pick baru,
prioritas PALING AWAL — lihat komentar urutan pick di `handle_sketch_
input`) memutuskan "klik ini editing fitur X" murni dari
`jarak(hit_point, anchor_fitur) <= radius_fitur * 1.5 + tol` — reach
IKUT MEMBESAR seiring radius rounding. Sejak "Perbaikan susulan 4"
menghapus formula precheck (radius sekarang bisa mendekati ukuran
objeknya sendiri, itu tujuannya), reach `radius * 1.5` bisa jauh lebih
besar dari objek — titik di wajah DATAR manapun jadi "terjangkau"
walau jelas bukan bagian permukaan lengkung rounding-nya, MENELAN
prioritas pick vertex/edge BARU di sudut lain (yang dicoba SETELAH
`round_edit`, cuma jalan kalau `round_edit.is_none()`).

Perbaikan (`find_round_feature_near`): tambah parameter `surface_kind:
SurfaceKind` (dari `FaceHit` hasil `face_pick_3d`, sudah dihitung
pemanggil), gate PALING AWAL — wajah `Plane` (datar) langsung `None`
SEBELUM tes jarak `reach` sama sekali, karena permukaan blend rounding
mana pun SELALU melengkung (`Sphere` utk vertex fillet 3-arah,
`Cylinder` utk sweep 1 tepi) — wajah datar TIDAK PERNAH bisa jadi bagian
rounding manapun, independen dari seberapa besar `reach`-nya. Klik di
wajah lengkung TETAP dites jarak seperti biasa (perilaku "klik ulang
rounding yang sudah ada utk edit radius" tidak berubah).

`cargo build --workspace` bersih, `cargo test --workspace --
--test-threads=1` tetap 153 test hijau (perubahan murni di jalur klik
`cadraw-app`, tidak menyentuh kernel).

## Status Sketch — Rantai Garis Kontinu (dikerjakan)

Permintaan user: di tool Garis mode sketch, klik-klik berturut harus
langsung menyambung dari titik sebelumnya (bukan minta 2 klik baru tiap
segmen), selesai kalau klik balik ke salah satu titik AWAL rantai atau
tekan ESC, dan tetap tampilkan nominal jarak sejajar garisnya seperti
sebelumnya.

Sebelumnya tool Garis adalah tool 2-klik generik lewat
`on_click_point`/`finish_multipoint` (jalur yang sama dgn Rectangle/
Circle/dst) — begitu 2 titik terkumpul, 1 `Entity::Line` di-commit dan
`pending_points` di-reset total, jadi klik berikutnya mulai garis baru
yang TIDAK nyambung. Preview dimensi live (leader lines panah +
label rotasi sejajar garis, sudah ada dari sebelumnya lewat
`sketch_render::dimension_leader_lines` + `CanvasHud::render_dimension_
pill`) sudah keyed off `pending_points.len() == 1`, jadi TIDAK perlu
kode render baru — tinggal jaga `pending_points` tetap 0-atau-1 elemen
sepanjang rantai supaya preview itu otomatis nyala tiap segmen.

- `CadrawApp`: field baru `line_chain_start: Option<DVec2>` (titik AWAL
  rantai, terpisah dari `pending_points[0]` yang berarti "titik akhir
  segmen terakhir / awal segmen berikutnya") + `line_chain_segments: u32`
  (penghitung segmen ter-commit, dipakai syarat minimal 2 segmen sebelum
  klik-balik-ke-awal dianggap "tutup loop" — supaya loop tertutup minimal
  segitiga, bukan bolak-balik 1 garis). Di-reset di `set_tool` dan di
  cabang ESC yang sudah ada (bareng `pending_points.clear()`).
- `ToolKind::Line` dikeluarkan dari arm dispatch klik generik (yang masih
  dipakai Rectangle/Circle/Ellipse/Arc/Measure/MeasureAngle) ke method
  baru `handle_line_chain_click(p, close_tol)`: klik pertama cuma catat
  titik awal; klik berikutnya SELALU commit 1 `Entity::Line` dari titik
  terakhir ke titik klik (lewat `InsertEntities` + `execute_sketch_
  command`, undo-able per segmen), lalu:
  - kalau klik menyentuh `line_chain_start` (toleransi = `tol` yang sama
    dgn `find_snap`) DAN sudah ≥2 segmen ter-commit → tutup PERSIS ke
    titik awal rantai (bukan titik klik mentah, biar loop benar-benar
    nyambung walau klik meleset dalam toleransi), lalu rantai selesai;
  - selain itu → rantai lanjut, `pending_points` diisi ulang 1 elemen
    (titik baru) supaya siap jadi awal segmen berikutnya.
  - klik di tempat (hampir) sama dgn titik terakhir diabaikan (guard
    `LINE_CHAIN_DEGENERATE_EPS`), tidak membuat segmen nol-panjang.
- `finish_multipoint`/`required_points`: arm `ToolKind::Line` dihapus
  (Line tidak pernah lewat sana lagi) — masing-masing dapat komentar
  penunjuk ke `handle_line_chain_click` supaya tidak disangka lupa
  di-handle.
- `status_text()`: hint tool Garis dipecah 3 kondisi (0 titik / sudah
  ≥2 segmen jadi bisa ditutup / masih <2 segmen) supaya user tahu opsi
  tutup-loop begitu tersedia.

Preview segmen yang sedang digambar (leader lines + pill panjang di
`build_overlay_lines`/`dynamic_input_ui`, keyed `pending_points.len()
== 1`) dan checkbox "Tampilkan Semua Ukuran" (`show_all_dimensions`,
independen) TIDAK disentuh — otomatis bekerja untuk tiap segmen rantai
tanpa perubahan.

`cargo build -p cadraw-app` bersih (tanpa warning baru).
`cargo test -p cadraw-sketch -p cadraw-kernel` tetap hijau (perubahan
murni di `cadraw-app`, tidak menyentuh kernel/sketch) — `cadraw-app`
sendiri tidak punya unit test (layer egui, divalidasi manual: gambar
segitiga tertutup lewat klik-balik-ke-titik-awal, dan polyline terbuka
diselesaikan ESC).

## Fix — Crash Extrude Dekat Wajah Rounding (dikerjakan)

Laporan user: aplikasi CLOSE (bukan hang, bukan panic Rust — tidak ada
pesan/backtrace apa pun di terminal, log debug hanya berhenti tiba-tiba
tepat setelah face pick sukses) saat extrude wajah yang tepi/sudut
TETANGGANYA sudah di-rounding (fillet).

Root cause: `Shape::union`/`subtract` (`vendor/opencascade-0.2.0/src/
primitives/shape.rs`) dan `AdHocShape::union`/`subtract`/`intersect`
(`vendor/opencascade-0.2.0/src/adhoc.rs`) — dipanggil `cadraw-kernel::
union`/`subtract`/`intersect`/`extrude_face` (jalur planar extrude fuse/cut
prism baru ke shape lewat boolean OCCT) — memanggil `BRepAlgoAPI_Fuse`/
`Cut`/`Common` MENTAH tanpa `IsDone()`/try-catch. Fuse/cut prism baru ke
solid yang tepinya sudah di-fillet adalah kasus OCCT yang bisa gagal
(`StdFail_NotDone`) di titik tangent dgn blend surface fillet. `StdFail_
NotDone` turunan `Standard_Failure`, BUKAN `std::exception` — lolos lewat
cxx dan memicu `std::terminate` (abort SELURUH proses), bukan `Result::
Err`. Kelas bug yang SAMA PERSIS sudah pernah diperbaiki utk fillet/chamfer
(lihat `vendor/README.md` "Perubahan #8"/"#3" — user drag radius rounding
sampai batas ujung objek) tapi belum pernah diterapkan ke boolean
union/subtract/intersect, satu-satunya celah yang tersisa.

Fix (lihat `vendor/README.md` "Perubahan #9" untuk `opencascade-0.2.0` &
"Perubahan #4" untuk `opencascade-sys-0.2.0`, detail lengkap di sana):
- `opencascade-sys`: 6 binding baru (`BRepAlgoAPI_Fuse/Cut/Common_ctor_
  checked`/`_shape_checked`) — try/catch(Standard_Failure) di
  `wrapper.hxx`, BEDA dari fillet/chamfer: bukan cuma `.Shape()`, ctor
  2-argumen kelas ini jalan eager (langsung Build() algoritma BOP), jadi
  ctor-nya JUGA dibungkus (ditulis manual, bukan lewat `construct_unique`
  generik cxx yang tidak bisa di-try/catch).
- `opencascade-0.2.0`: `Error::BooleanOpFailed(String)` baru;
  `Shape::union`/`subtract` & `AdHocShape::union`/`subtract`/`intersect`
  sekarang balikin `Result<..., Error>` (dulu tanpa jaminan sukses),
  lewat binding checked di atas.
- `cadraw-kernel`: `union`/`subtract`/`intersect`/`extrude_face`
  `.context(...)?`-kan `Result` baru itu (tanda tangan publiknya sendiri
  sudah `Result<KernelShape>` dari awal, jadi TIDAK ada perubahan API yang
  terlihat pemanggil).
- `cadraw-app`: NOL perubahan — semua titik panggil `cadraw_kernel::union/
  subtract/intersect/extrude_face` sudah pakai `match`/`if let Ok`/`.ok()?`
  dari awal, jadi begitu native exception-nya ketangkep jadi `Err`, alur
  error-handling yang sudah ada otomatis kepakai.

Regresi dibuktikan test baru `extrude_face_adjacent_to_fillet_does_not_
crash` di `cadraw-kernel` — fillet 1 tepi vertikal box 30×20×15 (radius
8mm), extrude wajah yang bertetangga LANGSUNG dgn tepi ter-fillet: klaim
intinya proses harus TETAP HIDUP (hasil `Ok` atau `Err` sama-sama
membuktikan tidak crash).

`cargo build --workspace` bersih. `cargo test --workspace -- --test-
threads=1` 154 test hijau (naik dari 153 — 70 di antaranya di
`cadraw-kernel`, naik dari 69, tambahan test regresi di atas).

## Status — Aktivasi Bidang Sketsa Langsung dari Viewport (dikerjakan)

Permintaan user: di mode sketch, 3 bidang (Top/Front/Right) sudah otomatis
menampilkan entitas sketsanya sekaligus (yang non-aktif digambar redup,
lihat loop `0..3` di `build_overlay_lines`), tapi belum ada cara memilih
bidang mana yang aktif langsung dari viewport 3D — cuma lewat dropdown
top-bar atau Items Drawer. Diminta: `Cmd + Klik` di desktop utk
mengaktifkan bidang yang diklik, plus gesture setara utk iPad (belum ada
keyboard fisik).

Pendekatan (bukan grid rapat 500-unit penuh utk bidang non-aktif — akan
membuat viewport penuh garis karena ketiganya saling tegak lurus & berpotongan
di origin):
- `cadraw-render/src/grid.rs`: `plane_outline()` baru — kerangka persegi
  tipis + silang sumbu kecil (bukan grid rapat), area `INACTIVE_PLANE_
  HALF_EXTENT` (120 unit) dipakai SEKALIGUS utk digambar & utk hit-test,
  jadi area yang divisualisasikan selalu sama persis dgn area yang bisa
  diklik.
- `cadraw-app/src/main.rs`: `build_overlay_lines` menggambar `plane_outline`
  utk 2 bidang non-aktif (cuma saat `is_sketching`), warna biru redup
  (senada `ACCENT_BLUE`) sbg penanda "ini bisa diklik".
- `pick_inactive_plane_at_cursor` (wrapper screen→ray) + `pick_inactive_
  plane_for_ray` (murni matematika, dipisah supaya testable tanpa kamera/
  layar, gaya sama dgn test `SketchPlane::ray_intersection` di `plane.rs`)
  — ray-test ke 2 bidang non-aktif, dibatasi `INACTIVE_PLANE_HALF_EXTENT`,
  menangkan yang titik potongnya paling dekat kamera kalau kena keduanya.
- `handle_plane_activation` (dipanggil paling awal di `handle_sketch_input`,
  sebelum klik jatuh ke tool/seleksi biasa):
  - **Desktop / iPad+keyboard eksternal**: `response.clicked() && modifiers.
    command` → `pick_inactive_plane_at_cursor` → `activate_plane_from_
    viewport` (pembungkus `set_sketch_plane` + toast status). Modifier
    `Cmd` diteruskan sama oleh egui/winit dari keyboard eksternal iPad,
    jadi jalur ini otomatis jalan di sana tanpa kode khusus.
  - **iPad sentuh murni (tanpa keyboard)**: "two-finger tap" — dilacak
    lewat `ui.input(|i| i.multi_touch())`, state terakhir `num_touches==2`
    disimpan (`two_finger_tap_press: Option<egui::MultiTouchInfo>`), begitu
    sentuhan dilepas (`multi_touch()` balik ke `None`) dicek `elapsed <=
    0.3s` & `center_pos` tidak bergeser jauh dari `start_pos` (bukan
    drag/pan) → aktifkan bidang di posisi itu. Dipilih krn belum diklaim
    gestur lain: 2-jari **drag** = pan/orbit, pinch = zoom, long-press
    1-jari = radial tool menu (`handle_radial_menu`, sudah ada sejak
    Fase 4).

Keputusan desain (default dipertahankan konsisten dgn dropdown/Items
Drawer yang sudah ada): aktivasi bidang lewat klik viewport TETAP men-snap
kamera menghadap bidang baru (`camera.orient_to_plane`, lewat `set_sketch_
plane` yang sama).

Keterbatasan yang diketahui & didokumentasikan di kode:
- Jalur two-finger tap ditulis & diuji SECARA LOGIS (5 test baru di
  `plane_activation_tests`, ray buatan tangan gaya `plane.rs`), tapi belum
  bisa diverifikasi end-to-end di iPad fisik — Fase 6 (build iOS) masih
  blocked di level linking OCCT utk aarch64-apple-ios (lihat status Fase 6
  di atas), jadi tidak ada device/simulator utk smoke-test gesture-nya.
- Deteksi tap tidak membedakan pinch-zoom-di-tempat (centroid nyaris diam
  tapi jari-jari bergerak menjauh/mendekat) dari tap murni — kalau user
  pinch cepat lalu lepas dlm <0.3s persis di area bidang non-aktif, bisa
  salah kepicu aktivasi bidang. Cukup jarang & bukan destruktif (tinggal
  klik/tap ulang plane semula), ditandai sbg follow-up kalau jadi masalah
  nyata setelah Fase 6 rilis & bisa dites di device asli.

3 test baru di `cadraw-render::grid` (vertex count, batas half-extent,
warna) + 5 test baru di `cadraw-app::plane_activation_tests` (hit Front/
Right plane, bidang aktif tidak pernah dikembalikan, hit di luar batas
diabaikan, ray sejajar kedua bidang → `None`). `cargo build --workspace`
& `cargo clippy -p cadraw-render -p cadraw-app --no-deps` bersih (nol
error, warning yang muncul semuanya pra-existing/gaya `map_or` yang sudah
dipakai di seluruh file, bukan regresi). `cargo test --workspace
-- --test-threads=1` 162 test hijau (naik dari 154 — 8 test baru di atas).

**Follow-up sama hari: diperluas ke mode 3D** (user: "tolong multi layer
ini juga di terapkan di 3D mode"). Sebelumnya outline bidang non-aktif &
gestur Cmd+Klik/two-finger tap cuma jalan selagi `is_sketching`. Kedua
guard `if self.is_sketching` itu dilepas:
- `build_overlay_lines` sekarang menggambar `plane_outline` di KEDUA
  mode, dgn alpha lebih redup di mode 3D (0.18 vs 0.30 saat sketching)
  supaya tidak bersaing visual dgn body solid yang sedang dilihat.
- `handle_plane_activation` sekarang jalan tanpa syarat mode — klik/tap
  sebuah bidang dari mode 3D otomatis "lompat masuk" ke sketch di bidang
  itu (lewat `set_sketch_plane` yang sama, yang sudah menyalakan
  `is_sketching` sendiri sejak awal — tidak perlu logic baru), identik
  dgn cara dropdown/Items Drawer bekerja. Aman dijalankan bersamaan dgn
  face/edge/vertex picking mode 3D yang sudah ada karena `Cmd` tidak
  pernah dipakai gesture pick manapun di sana (dicek ulang sebelum
  diterapkan, bukan asumsi) — tapi tetap ditempatkan SETELAH early-return
  `picking_mode != PickMode::None` di `handle_sketch_input`, supaya tidak
  mengganggu sesi pick edge/face fillet yang sedang berlangsung.

Tidak ada API/test baru yang perlu diubah — 5 test `pick_inactive_plane_
for_ray` murni geometri, tidak bergantung `is_sketching` sama sekali.
`cargo build`/`clippy`/`test` (`-p cadraw-app -p cadraw-render`) tetap
bersih, 28 test itu semua masih hijau.

## Status — Drag Axis X/Y (Sketch 2D) & X/Y/Z (Body 3D) (dikerjakan)

Permintaan awal ("drag sketch 2D dan object 3D ke arah X/Y/Z") awalnya
diperluas jadi fitur elevasi Z per-entitas sketch (`Sketch::elevations`) —
supaya dua entitas yang "ditumpuk" (mis. lingkaran luar & dalam utk profil
"gelas") bisa dipilih terpisah lewat elevasi berbeda. **Elevasi ini
DICABUT TOTAL setelah dianalisis ulang dan dibandingkan dengan SolidWorks/
AutoCAD/Shapr3D** (atas permintaan eksplisit user): sketch di ketiga tool
itu SELALU murni planar (SolidWorks: 1 sketch = 1 plane, titik, tanpa
kecuali) — menumpuk profil di ketinggian berbeda semestinya lewat *offset
sketch plane* baru (bidang eksplisit, numerik, terlihat di panel), BUKAN
field Z tersembunyi per-entitas tanpa input angka. Lebih penting lagi:
kasus "gelas" itu sendiri TIDAK butuh elevasi sama sekali — cara
profesionalnya adalah Shell/Hollow (sudah ada sejak Fase 3) atau Boolean
Subtract dua body hasil extrude terpisah (juga sudah ada), keduanya cuma
butuh bisa MEMILIH lingkaran dalam secara terpisah dari lingkaran luar,
bukan menggesernya ke Z lain. Root cause sebenarnya murni masalah
**seleksi entitas tumpang-tindih**, diselesaikan oleh klik-cycle di bawah
— fondasi elevasi jadi kerumitan arsitektur (data model + serde + hit-
test + render) tanpa manfaat nyata (YAGNI), makanya dicabut.

Yang **bertahan** dari putaran ini (fondasinya benar & sudah lolos
perbandingan dgn CAD standar):

- [x] **Klik-cycle seleksi entitas tumpang-tindih** (`hit_test_cycled`,
      state `CadrawApp::last_select_click`) — klik ulang dalam radius
      `SELECT_CYCLE_CLICK_PX` (4px) dari klik sebelumnya memilih kandidat
      berikutnya di antara entitas yang sama-sama ke-hit dalam toleransi,
      bukan selalu yang pertama/terdekat. **Preseden nyata**: AutoCAD
      punya fitur resmi bernama *Selection Cycling* (`SELECTIONCYCLING`
      sysvar) persis untuk masalah ini; Fusion 360/SolidWorks punya
      varian serupa. Menggantikan `Sketch::hit_test` di 4 titik panggilan
      lama (Pilih/Offset/Trim hover + klik seleksi).
- [x] **`translate_entity`/`TranslateEntities`** (`cadraw-sketch`) — geser
      entitas sepanjang bidang lokalnya (u,v), command undo-able berbasis
      delta. Ini yang menjawab permintaan asli "drag sketch ke arah X/Y".
- [x] **Gizmo drag axis X/Y sketch** (U/V bidang aktif) — 2 panah,
      auto-muncul saat tool Pilih aktif & ada seleksi sketch (tanpa body
      terpilih), lewat widget `CanvasHud::render_draggable_double_arrow_handle`
      yang sudah ada (pola vertex/edge fillet gizmo).
- [x] **`cadraw_kernel::translate_shape(shape, dx, dy, dz)`** — TIDAK
      butuh binding OCCT baru: `opencascade-0.2.0` vendor sudah punya
      `Shape::set_global_translation` sejak awal, tinggal dibungkus
      fungsional (`deep_clone` + set translation) pola sama `fillet_all`/
      `chamfer_all`, jauh lebih murah (cuma `Location`, B-rep tak disentuh).
- [x] **Gizmo drag axis X/Y/Z body 3D** — 3 panah dunia murni di pusat
      bounding-box, auto-muncul saat tool Pilih aktif & PERSIS satu body
      terpilih (pola sama `active_face`/`active_vertex`/`active_edge`).
      Live preview tiap frame drag (`translate_shape` dihitung ulang dari
      shape ASLI + delta akumulasi, bukan chaining, konsisten dgn semua
      preview gizmo lain di `build_combined_body_mesh`); commit lewat
      `ReplaceGeometryCommand` yang SUDAH ADA (tidak ada command model
      baru). Ini yang menjawab permintaan asli "drag object 3D ke X/Y/Z".
- [x] 165 test lulus di seluruh workspace (2 test baru bersih di
      `cadraw-sketch` utk `translate_entity`/`TranslateEntities`, 1 di
      `cadraw-kernel` utk `translate_shape` — test elevasi yg sempat ada
      sudah ikut dihapus bersama fiturnya). `clippy -D warnings` masih
      gagal di file yang TIDAK disentuh fitur ini (`cadraw-ui`,
      `cadraw-sketch::region`) — bug lint pre-existing di `main`,
      dikonfirmasi via `git stash`, bukan regresi dari sini.
- [ ] **Sengaja belum ada**: popup daftar kandidat visual utk klik-cycle
      (AutoCAD/SolidWorks tampilkan flyout, punya CADRAW masih "buta" —
      klik berulang tanpa indikator jumlah/kandidat aktif); gizmo
      translate multi-body sekaligus (cuma 1 body per drag); sketch
      benar-benar planar per definisi (sesuai SolidWorks) — kalau nanti
      butuh profil di ketinggian berbeda, jalur yang benar adalah *offset
      sketch plane* baru (numerik, terlihat di panel), bukan Z per-entitas
      — belum diimplementasikan, dicatat sebagai arah yang benar utk masa
      depan; verifikasi visual/UX sungguhan di device — sama seperti semua
      fitur viewport lain, belum bisa dicek dari sandbox agent.

### Revisi — Gizmo geser sketch: 1 handle "+" omnidirectional + snap + nudge keyboard

User: "sekarang kan ada 2 Icon, tolong ubah jadi 1 icon aja di tengah dg
logo + ... kalo aku mau satukan sketch dg pusat yang sama pakai titik +
ini juga" — 2 panah U/V terpisah (masing-masing 1 sumbu) diganti 1 handle
bulat "+" di centroid seleksi, digeser bebas ke u DAN v sekaligus.

- [x] **`CanvasHud::render_draggable_move_handle`** (`cadraw-ui`) — widget
      baru, sibling `render_draggable_double_arrow_handle`: lingkaran kuning
      +ikon "+" (bukan panah 2 sisi, karena drag-nya omnidirectional bukan
      1 sumbu), cursor `Move`. Warna sengaja beda dari biru gizmo Extrude
      supaya tidak tertukar walau anchor keduanya bisa berdekatan.
- [x] **State disederhanakan**: `sketch_axis_drag: Option<SketchDragAxis>` +
      `sketch_axis_delta: f64` (per-sumbu) → `sketch_move_dragging: bool` +
      `sketch_move_delta: DVec2` (gabungan u,v). Enum `SketchDragAxis`
      dihapus total (tidak perlu lagi, cuma 1 handle).
- [x] **Mekanik drag**: `dynamic_input_ui` proyeksikan `drag_delta` piksel
      yang SAMA ke u_axis DAN v_axis lewat `project_screen_drag_to_world_axis`
      dua kali (bukan solve simultan 2D) — akurat pas kamera tegak lurus
      bidang (kasus umum: mode sketsa selalu mengorientasikan kamera
      begitu), sedikit shear di sudut oblique, trade-off yang sama dgn
      semua gizmo single-axis lain di file ini.
- [x] **Snap-to-point selagi drag** — posisi geser saat ini diuji lewat
      `find_snap` (snap engine yang sudah ada sejak Fase 1, endpoint >
      midpoint > center > intersection > grid) tiap frame drag; kalau kena,
      delta dijepret PERSIS supaya titik "+" nempel di titik itu. Inilah
      jawaban permintaan "satukan pusat sketch": drag lingkaran A sampai
      "+"-nya nempel ke center lingkaran B → kedua pusat jadi identik.
- [x] **Commit jadi 1 command** (bukan 2 command X lalu Y terpisah) —
      `commit_sketch_move_drag` kirim `TranslateEntities` sekali dengan
      delta `DVec2` gabungan, 1 langkah undo per drag.
- [x] **Nudge keyboard** (user, mid-turn: "pakai CMD + anak panah") —
      Cmd/Ctrl (`modifiers.command`, sudah cross-platform di egui) + Panah
      Kiri/Kanan/Atas/Bawah menggeser seleksi 1.0 mm (`NUDGE_STEP_MM`)
      sepanjang u/v, commit LANGSUNG per tekan (bukan diakumulasi seperti
      drag mouse) — reuse `TranslateEntities` yang sama, ditaruh di blok
      shortcut `!text_focused` yang sudah ada (sebelah Delete/Backspace).
- [x] Workspace tetap hijau — `cargo build --workspace` bersih tanpa
      warning baru, 165 test tetap lulus (perubahan ini murni UI/interaksi,
      tidak ada test unit baru).

### Revisi 2 — Handle "+" bawaan mode sketsa (bukan tool Pilih) + blend visual + armed-nudge + snap ke pusat objek lain

User: "select mode itu ada di 3D mode aja... icon + ini ada bawaan di
sketch aja... titik + kalo bisa menyatu dg sketch 2d jangan berupa icon +
warna kuning... kalo mau memindahkan bisa klik-drag ATAU klik + (masuk
mode geser) lalu digeser dg keyboard anak panah... titik + jadi titik
untuk menyatukan 2 sketch atau lebih" — 4 perubahan atas revisi
sebelumnya, semua di `dynamic_input_ui`/`handle_sketch_input`
(`cadraw-app`) + `render_draggable_move_handle` (`cadraw-ui`):

- [x] **Lepas gate `tool == Select`** dari `selected_entities_centroid`
      (satu-satunya konsumen: gizmo geser sketch) — diganti `is_sketching`.
      `self.selected` sendiri sudah persist lintas tool (`set_tool` tidak
      pernah membersihkannya), jadi cukup itu supaya handle "+" tetap
      hidup & draggable walau user pindah ke tool 2D lain (Line/Rectangle/
      dst) tanpa perlu balik ke tool Pilih dulu. `selected_closed_region_
      centroid` (dipakai gizmo Extrude, konsumen BEDA) sengaja TIDAK
      disentuh — tetap gate `tool == Select` seperti semula, supaya arrow
      Extrude tidak nongol di tool manapun. Sisi 3D (`selected_single_body_
      center`, gizmo drag axis X/Y/Z body) juga tidak disentuh — "select
      mode" tetap relevan penuh di situ, sesuai permintaan user.
- [x] **Restyle `render_draggable_move_handle`** — badge lingkaran kuning
      solid + ikon "+" putih tebal diganti crosshair tipis warna biru
      (`rgb(77,166,255)`, selaras `COLOR_SELECTED` di `cadraw-render/
      sketch.rs`) tanpa fill solid saat idle, supaya "menyatu" dgn garis
      sketch alih-alih tampak sbg tombol UI berdiri sendiri. Tetap ada glow
      ring saat hover/drag dan cincin denyut saat armed supaya tidak
      hilang kegunaannya sbg target klik.
- [x] **Mode geser "armed"** (`sketch_move_armed: bool`, field baru) —
      klik SINGKAT (bukan drag) di handle "+" meng-arm; selama armed,
      Panah Kiri/Kanan/Atas/Bawah **POLOS** (tanpa Cmd) menggeser seleksi
      1 mm per tekan, commit lewat `TranslateEntities` yang sama dgn jalur
      Cmd+Panah lama (yang TETAP ada, gate-nya ikut dilonggarkan dari
      `tool == Select` ke `is_sketching`). `Sense::drag()` di handle diganti
      `Sense::click_and_drag()` supaya `clicked()` (armed toggle) terbaca
      terpisah dari `dragged()` (drag_started otomatis melucuti armed —
      drag menang). Direset di semua titik reset serupa yg sudah ada:
      `set_tool` (central), Escape (prioritas SEBELUM clear seleksi), klik
      viewport apa pun (ganti seleksi), Delete/Backspace.
- [x] **`region_center_snap`** (free function baru, `cadraw-app`,
      ditambah 3 test) — perluasan snap selagi drag: kalau `find_snap`
      (endpoint/midpoint/center/intersection/grid — titik DOF entitas
      TUNGGAL) tidak kena, dicoba juga titik tengah region tertutup LAIN
      di sketch yang sama (`find_closed_regions`, dikecualikan region milik
      seleksi sendiri). Inilah "satukan 2 sketch/profil" yang diminta:
      pusat rectangle 4-garis atau profil majemuk bukan titik snap entitas
      manapun, jadi butuh sumber terpisah dari `find_snap`.
- [x] Workspace hijau — `cargo build --workspace` bersih, 168 test lulus
      (165 lama + 3 baru utk `region_center_snap` di `cadraw-app`).

### Revisi 3 — Handle "+" tanpa perlu seleksi eksplisit sama sekali (fallback seluruh sketch) + catatan bug Cmd+Panah kiri/kanan

User: "jangan gunakan tool pilih untuk mengaktifkan + ini, dia muncul
terus aja... Trus aku coba geser dg CMD + anak panah keyboard yang bisa
baru keatas dan kebawah, anak panah kiri kanan belum bisa" — Revisi 2
masih mengharuskan MINIMAL 1x klik-seleksi lewat tool Pilih supaya
`self.selected` terisi (baru setelah itu handle "+" bawaan lintas-tool).
User mau lebih jauh: TIDAK butuh seleksi eksplisit APAPUN.

- [x] **`CadrawApp::move_target_ids`** (helper baru) — seleksi eksplisit
      (`self.selected`) kalau ADA, atau SEMUA entitas di sketch aktif kalau
      TIDAK ada seleksi sama sekali. Fallback "seluruh sketch" inilah yang
      bikin handle "+" langsung muncul begitu sketch punya isi — tanpa
      perlu tool Pilih SAMA SEKALI (beda dari Revisi 2, yang cuma
      melonggarkan gate SETELAH ada seleksi). Tool Pilih tetap berguna
      untuk kasus presisi: geser SEBAGIAN sketch saja.
- [x] `selected_entities_centroid` diganti nama jadi
      **`sketch_move_target_centroid`** (dan isinya pakai `move_target_ids`)
      supaya nama fungsi tidak lagi menyesatkan (sekarang bisa mewakili
      SELURUH sketch, bukan cuma "seleksi"). `find_region_center_snap`
      (exclude-nya) dan `commit_sketch_move_drag`/nudge (ids-nya) ikut
      dipindah ke `move_target_ids` — cuma 4 titik pakai, jadi rename +
      migrasi murah. `find_region_center_snap` saat fallback "seluruh
      sketch" otomatis TIDAK punya region lain tersisa buat disnap
      (semuanya masuk exclude) — masuk akal: "satukan 2 profil" tetap
      butuh seleksi manual salah satunya dulu, supaya yang SATU LAGI
      tersisa jadi target snap.
- [x] Gate nudge (Cmd+Panah & armed+panah-polos) dilonggarkan dari
      `!self.selected.is_empty()` jadi `!self.sketch().entities.is_empty()`
      — konsisten dgn fallback di atas.
- [x] **Investigasi bug "Cmd+Panah kiri/kanan tidak jalan, atas/bawah
      jalan"**: dibaca ulang kode nudge baris-per-baris — 4 cabang
      (Left/Right/Up/Down) simetris persis, semua lewat `TranslateEntities`
      yang sama (`translate_entity` di `cadraw-sketch` juga simetris x/y,
      sudah ada test `translate_entity_shifts_all_variants` yang lulus).
      Tidak ditemukan kode lain di seluruh workspace yang mengonsumsi
      `ArrowLeft`/`ArrowRight` duluan (`consume_key` tidak dipakai di mana
      pun — egui `key_pressed` sendiri tidak "React tag project" style
      konsumsi event, jadi tidak ada tabrakan internal). Dicek juga
      shortcut tiling jendela default macOS Sequoia (websearch) — BUKAN
      Cmd+Panah (defaultnya Fn-Control-Panah), jadi bukan itu. Kesimpulan:
      kemungkinan besar bentrok keybinding level OS/aplikasi pihak ketiga
      di mesin user (mis. app snapping jendela custom yang bind
      Cmd+Kiri/Kanan), BUKAN bug kode CADRAW — tidak bisa diverifikasi
      lebih lanjut dari sandbox (tidak ada display, sudah didokumentasikan
      berkali-kali di memory project). Mitigasi konkret: jalur BARU "klik +
      lalu panah POLOS tanpa Cmd" (armed mode, dari Revisi 2) sama sekali
      tidak butuh Cmd, jadi otomatis kebal dari kemungkinan bentrokan ini —
      direkomendasikan ke user sbg jalur utama.
- [x] Workspace hijau — `cargo build --workspace` bersih, `clippy -p
      cadraw-app` tidak ada warning baru di kode yang disentuh, 168 test
      tetap lulus (murni refactor/rename, tidak ada logika baru yang perlu
      test baru).

### Revisi 4 — SATU handle "+" PER OBJEK (bukan 1 titik gabungan), tool Pilih tetap ada di sketch

User (dgn screenshot): "Masih ngaco. Aku bikin 2 Sketch. Titik + jadi ada
diantara 2 sketch ini. Kan harusnya masing2 punya. dan selalu muncul
terus." — Revisi 3 fallback ke SATU centroid gabungan SELURUH sketch saat
tidak ada seleksi, jadi 2 rectangle terpisah malah dapat 1 titik "+" di
tengah-tengah keduanya (bukti di screenshot). Ditanya juga soal
"pindahkan tool Pilih ke 3D aja" — dikonfirmasi via `AskUserQuestion`:
tool Pilih TETAP ada & dipakai di sketch (Extrude/Mirror/Revolve/Delete/
Constraint semua masih butuh dia), cuma memang sudah TIDAK wajib buat
geser "+".

- [x] **Desain ulang total dari "1 target gabungan" jadi "banyak grup
      independen"**: `CadrawApp::sketch_move_groups()` — kalau ada
      seleksi eksplisit (`self.selected`, tool Pilih), itu SATU-SATUNYA
      grup (jalur presisi, tidak berubah); kalau TIDAK ada seleksi, SATU
      GRUP PER REGION TERTUTUP (`find_closed_regions`, bukan digabung jadi
      satu lagi) — 2 rectangle terpisah = 2 grup = 2 titik "+" independen.
      `move_target_ids`/`sketch_move_target_centroid` (fallback-gabungan,
      Revisi 3) DIHAPUS total, diganti `group_centroid(ids)` per-grup.
- [x] State `sketch_move_target: Option<HashSet<EntityId>>` (baru) —
      identitas grup MANA yang sedang di-drag/di-armed sekarang (bukan
      index/posisi, supaya tetap valid walau urutan `find_closed_regions`
      berubah antar frame). `sketch_move_dragging`/`sketch_move_armed`
      sekarang berlaku KHUSUS untuk grup ini; handle lain tetap statis &
      independen. Direset (bersama armed) di SEMUA titik reset yang sudah
      ada: `set_tool`, Escape, klik viewport, Delete/Backspace,
      `commit_sketch_move_drag`.
- [x] **Render loop di `dynamic_input_ui` diubah dari "1 blok if" jadi
      "loop per grup"** — tiap grup dari `sketch_move_groups()` dapat
      handle "+" sendiri (posisi = `group_centroid` + delta HANYA kalau
      grup itu `sketch_move_target`), dibungkus `ui.push_id` dengan ID
      STABIL dari isi grupnya (`EntityId::data().as_ffi()`, di-sort lalu
      di-hash) — bukan urutan render — supaya drag di tengah gestur tidak
      putus kalau `find_closed_regions` menghitung ulang urutan region di
      frame berikutnya (region tanpa id eksplisit HANYA stabil kalau
      urutan render-nya juga stabil, yang tidak dijamin `find_closed_
      regions`, jadi ini bukan langkah opsional). Klik handle grup LAIN
      selagi satu grup masih armed langsung MEMINDAH target (bukan
      menumpuk armed di 2 grup).
- [x] **`nudge_target_ids()`** (baru, ganti `move_target_ids`) — target
      nudge KEYBOARD (Cmd+Panah / armed+panah-polos) HARUS satu grup yang
      tidak ambigu, beda dari render (bisa banyak handle sekaligus).
      Prioritas: seleksi eksplisit → grup yang sedang armed/di-drag →
      (kalau sketch cuma punya SATU region total, tidak ambigu sama
      sekali) region itu → selain itu `None` (sengaja tidak menggeser
      apa-apa; dgn 2+ region tak-berlabel, tidak ada cara tahu yang mana
      dimaksud tanpa user menunjuk salah satu "+" dulu).
- [x] **Tool Pilih TIDAK dihapus dari sketch** (dikonfirmasi via
      `AskUserQuestion`, opsi "Tetap ada, tapi tidak wajib") — masih
      dipakai penuh utk Extrude/Mirror/Revolve/Delete/Constraint seperti
      sebelumnya; TIDAK ADA perubahan di `left_toolbar.rs`.
- [x] Workspace hijau — `cargo build --workspace` bersih, `clippy -p
      cadraw-app` tidak ada warning baru di kode yang disentuh, 168 test
      tetap lulus (perubahan ini re-arsitektur interaksi/render, bukan
      logika kernel/sketch murni baru — cakupan test lama yg sudah ada
      utk `region_center_snap`/`translate_entity`/`find_closed_regions`
      tetap relevan & lulus).

### Revisi 5 — Fix drag "+" lepas dari kursor di kamera oblique (ray-plane intersection, bukan proyeksi 2-sumbu)

User: "udah enak kalo view nya dari top. Tapi kalo miring saat di drag
pointer di mouse berbeda dg + yang di geser." Root cause: mekanisme drag
lama memproyeksikan `drag_delta` piksel ke u-axis DAN v-axis SECARA
TERPISAH lewat `project_screen_drag_to_world_axis` lalu dijumlah — akurat
cuma kalau u/v tegak lurus sempurna DI LAYAR (kasus top-down), makin
shear/meleset dari posisi kursor sungguhan begitu kamera oblique (u/v
tidak lagi ortogonal di ruang layar, tapi kode menjumlahkan proyeksi
independennya seolah masih ortogonal — bukan solve simultan 2D, persis
seperti yang sudah diperingatkan di komentar lama sebagai trade-off).

- [x] **Ganti ke `screen_to_plane_point`** (ray-plane intersection yang
      SUDAH ADA & sudah dipakai menempatkan titik saat menggambar Line/
      Rectangle/dst) — tiap frame drag, ambil posisi kursor SAAT INI
      (`handle_resp.interact_pointer_pos()`), ray-cast LANGSUNG ke bidang
      sketsa aktif, dapat titik (u,v) EKSAK di bawah kursor. `sketch_move_
      delta` dihitung ABSOLUT (`titik_di_bidang - centroid`) tiap frame,
      bukan diakumulasi inkremental dari delta piksel lagi — jadi tidak
      ada ruang shear terakumulasi sama sekali, akurat untuk sudut kamera
      berapa pun (selama ray tidak sejajar bidang, kasus degenerate
      bawaan geometri bukan bug).
- [x] `project_screen_drag_to_world_axis` (proyeksi 2-sumbu lama) TIDAK
      dihapus — tetap dipakai gizmo 1D (body axis-drag X/Y/Z, face push/
      pull) di tempat lain, yang MEMANG gerak 1 sumbu jadi ray-plane tidak
      relevan di situ. Cuma handle "+" sketch (gerak bebas 2D di bidang)
      yang pindah teknik.
- [x] Workspace hijau — `cargo build --workspace` bersih, 168 test tetap
      lulus (murni ganti teknik proyeksi geometri, tidak ada logika baru
      yang perlu test unit baru — `screen_to_plane_point`/`ray_intersection`
      sendiri sudah dipakai & terverifikasi lewat jalur menggambar sejak
      Fase 1).

### Revisi 6 — Gizmo geser body 3D jadi satu "+" (X/Y drag bebas + Shift-lock Z, armed + nudge X/Y/Z)

User: "geser di mode 3D masih menampilkan 3 icon, tolong ini diubah
seperti + di 2D, tapi untuk 3D ini berarti bisa geser X, Y dan Z ya?"
Riset dulu ke pola industri (Shapr3D, SolidWorks 3DMOVE, AutoCAD Triad —
semuanya pakai 1 widget gabungan di pusat objek: bola/tile pusat = geser
bebas 2 sumbu, lengan = 1 sumbu terkunci) sebelum implementasi, supaya
tidak asal tiru "+" 2D murni (sketch 2D DOF, body 3D DOF — drag mouse
tetap cuma 2D jadi tetap butuh cara eksplisit utk sumbu ketiga).

- [x] **Body axis-drag 3-panah-terpisah dihapus total** — state
      `body_axis_drag: Option<Vec3>`/`body_axis_target`/`body_axis_delta:
      f64` diganti `body_move_target: Option<BodyId>`/`body_move_dragging:
      bool`/`body_move_delta: Vec3`/`body_move_armed: bool` (Vec3, bukan
      f64 — nampung X/Y/Z sekaligus, bisa campur drag X/Y lalu Shift-drag
      Z tanpa reset).
- [x] **Satu `render_draggable_move_handle`** (fungsi yang SAMA persis dg
      "+" sketch 2D, `crates/cadraw-ui/src/canvas_hud.rs`) di pusat body,
      gantikan loop `for dir in [X, Y, Z]` yang dulu render 3
      `render_draggable_double_arrow_handle` terpisah 24mm dari pusat.
- [x] **Drag bebas TANPA Shift = X/Y** (bidang datar dunia lewat pusat
      body) — pakai teknik ray-plane `screen_to_plane_point` yang SAMA
      dgn Revisi 5 (akurat di sudut kamera manapun), lewat
      `SketchPlane { origin: center, ..SketchPlane::top() }` — SENGAJA
      TIDAK pakai `SketchPlane::from_origin_normal(center, Vec3::Z)`
      karena normal persis Z bikin basis `u_axis`/`v_axis` internalnya
      (`normal.cross(arbitrary)`) kolaps nol lalu `normalize()` NaN —
      bug laten yang ketemu pas mau reuse fungsi itu utk ground plane.
- [x] **Shift+drag = kunci Z** — pakai `project_screen_drag_to_world_axis`
      lama (genuinely 1D, ray-plane tidak relevan), sama fungsi yang dulu
      dipakai per-panah, cuma sekarang gerbangnya modifier bukan
      handle terpisah.
- [x] **Armed + keyboard nudge diperluas ke 3 sumbu** —
      Kiri/Kanan/Atas/Bawah = X/Y (mirror pola nudge sketch), tambah
      PageUp/PageDown = Z. Commit per tekan lewat `translate_selected_body`
      (bukan command sketch) — sudah undo-able lewat `ReplaceGeometryCommand`.
- [x] `body_move_armed`/`body_move_target` dilucuti di semua titik yang
      dulu melucuti `sketch_move_armed` (Escape, `set_tool`, klik viewport
      re-select, `delete_selected_bodies`) — konsisten dgn alasan yang
      sama: ganti konteks = niat "lagi fokus geser objek ini" sudah basi.
- [x] Workspace hijau — `cargo build --workspace` bersih, 168 test tetap
      lulus (murni re-arsitektur interaksi/render gizmo body, tidak ada
      logika kernel/sketch baru yang perlu test unit baru).

## Status — Icon Gizmo Push/Pull Solid & Profesional (dikerjakan)

User: "design icon gizmo ini tolong buat lebih profesional. Aku mau
cukup 1 icon aja. Tapi yang benar2 fungsional. Yang 3D frame itu ok,
cuma harus kamu ubah jadi solid dan ukuran di kecilin. Karena yang icon
lingkaran 2D itu aneh." — screenshot menunjukkan gizmo push/pull face
tampil sbg DUA elemen tumpang tindih: kerucut-ganda WIREFRAME raksasa
(ukuran mm dunia tetap, jadi memenuhi layar saat kamera zoom in) + badge
lingkaran biru flat 2D di tengahnya (widget drag sungguhan). `/ecc:plan`
dijalankan dulu, sempat di-re-verifikasi terhadap 4 commit baru (fitur
gizmo geser objek "+" yang tidak berkaitan) sebelum eksekusi.

- [x] **`sketch_render::solid_double_arrow_gizmo_mesh`** (baru,
      `cadraw-render/src/sketch.rs`) — versi SOLID dari
      `double_arrow_gizmo_lines` lama (silhouette sama persis: poros +
      2 kepala kerucut), tapi segitiga flat-shaded (tiap segitiga vertex
      sendiri, normal tegas per-wajah = kesan "gem"/facet tajam) alih-
      alih rusuk kawat. Winding tiap segitiga dikoreksi otomatis lewat
      `outward_hint` supaya normal SELALU menghadap keluar (dipakai
      shading, beda dari sekadar soal visibility — `mesh_pipeline` pakai
      `cull_mode: None`).
- [x] **`SceneRenderer::set_gizmo_mesh`** (baru, `scene.rs`) — buffer
      GPU terpisah dari `set_mesh` (body CAD), tapi pipeline SAMA
      (`mesh_pipeline`/`fs_mesh`) supaya shading gizmo (ambient floor +
      rim light) konsisten dgn material body, bukan icon UI yang
      ditempel di atas scene. Digambar terakhir di `paint()`.
- [x] **`CadrawApp::build_gizmo_mesh`** (baru, `main.rs`) — SATU fungsi
      dipakai utk KEEMPAT jenis gizmo (extrude sketch, push/pull face,
      rounding vertex, rounding edge; beda cuma warna tint), ukurannya
      dihitung dari `pixel_tolerance_to_world` (mm per piksel layar)
      BUKAN konstanta mm tetap — akar bug ukuran raksasa di screenshot.
      Gizmo sekarang selalu ~sebesar ini di layar berapa pun jarak
      kamera (pola sama dgn manipulator Fusion 360/Blender).
- [x] 4 panggilan `double_arrow_gizmo_lines` (wireframe) dihapus dari
      `build_overlay_lines` (garis panduan putus-putus tetap ada, cuma
      icon panahnya yang pindah ke mesh solid).
- [x] `CanvasHud::render_draggable_double_arrow_handle` dilucuti —
      badge lingkaran biru + icon panah `▲-▼` flat-nya DIHAPUS total,
      sisa cuma hit-area `Sense::drag()` + logic cursor-icon (signature
      & `Response` TIDAK berubah, jadi 4 call site di `main.rs` tidak
      perlu disentuh). Hasil akhir: **1 icon per gizmo** — solid,
      ter-shading, kecil & proporsional di semua level zoom.
- [x] 3 unit test baru (`sketch.rs`): triangle-soup valid (index dalam
      batas, tidak ada NaN, normal satuan), mesh kosong utk normal nol
      vektor (tidak panic), dan ukuran ikut membesar/mengecil sesuai
      `arrow_size` (jaminan dasar skala-berbasis-piksel benar-benar
      berefek).
- [x] Workspace hijau — `cargo build --workspace` bersih, 171 test
      lulus (168 lama + 3 baru), smoke-run `cargo run --bin cadraw`
      (8 detik) tidak crash/tidak ada validation error wgpu.

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
