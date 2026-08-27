/* =========================================================
   DuCAD Landing Page — i18n (English default, Indonesian toggle)

   Strategy: the HTML is authored in English (source of truth).
   On load we capture every [data-i18n] / [data-i18n-alt] /
   [data-i18n-aria] element's original English content into memory,
   then swap in the Indonesian dictionary below when requested.
   Switching back to English simply restores the captured original.
   ========================================================= */

window.ducadI18n = (() => {
  "use strict";

  const STORAGE_KEY = "ducad-lang";
  const DEFAULT_LANG = "en";

  /* ---------- Indonesian dictionary ---------- */
  const id = {
    "skip.link": "Langsung ke konten",

    "nav.fitur": "Fitur",
    "nav.semuaFitur": "Semua Fitur",
    "nav.mengapa": "Mengapa DuCAD",
    "nav.arsitektur": "Arsitektur",
    "nav.mulai": "Mulai",
    "nav.github": "GitHub",
    "nav.ctaMulai": "Mulai Sekarang",
    "nav.toggleAria": "Buka menu navigasi",

    "hero.eyebrow": "Design Universe CAD",
    "hero.h1": 'CAD 2D/3D modern, parametrik, dan <span class="text-gradient">berkinerja tinggi</span> — 100% Rust.',
    "hero.lead": 'DuCAD menggabungkan presisi penyusunan draf teknik 2D ala <strong>AutoCAD</strong> dengan kemudahan pemodelan langsung intuitif ala <strong>Shapr3D</strong> — didukung kernel solid modeling kelas industri <strong>OpenCASCADE (OCCT)</strong> dan akselerasi grafis modern <strong>WebGPU</strong>.',
    "hero.ctaPrimary": "Lihat di GitHub",
    "hero.ctaSecondary": "Jelajahi Fitur ↓",
    "hero.badge1": "Rust 2021 Edition",
    "hero.badge2": "OpenCASCADE (OCCT)",
    "hero.badge3": "wgpu / WebGPU",
    "hero.badge4": "egui / eframe",
    "hero.badge5": "MIT / Apache-2.0",
    "hero.visualAlt": "Tampilan aplikasi DuCAD menunjukkan panel Feature Tree & Riwayat parametrik di atas sebuah solid 3D",

    "stats.label1": "Modul fitur utama",
    "stats.label2": "Bahasa (i18n)",
    "stats.label3": "Konsumsi RAM",
    "stats.label4": "Waktu startup",
    "stats.label5": "Platform (macOS/Linux/Windows)",
    "stats.label6": "Lisensi dual (MIT/Apache-2.0)",

    "featHead.eyebrow": "Kemampuan Inti",
    "featHead.h2": "Satu aplikasi, alur kerja lengkap dari sketsa hingga manufaktur",
    "featHead.lead": "Dari sketsa 2D presisi hingga gambar kerja siap produksi — setiap tahap desain teknik tercakup dalam satu alur kerja yang konsisten.",

    "f1.chip": "01 · Sketsa 2D",
    "f1.h3": "Sketsa 2D parametrik &amp; geometri presisi",
    "f1.p": "Perangkat sketsa lengkap dengan solver kendala geometris/dimensional mandiri, sistem snapping bertingkat, dan modifikasi kurva interaktif — fondasi presisi untuk semua pemodelan 3D di atasnya.",
    "f1.li1": "Entitas lengkap: Line, Rectangle, Circle, Arc (Center-Radius &amp; 3-Point), Ellipse, Regular Polygon N-sisi, Slot",
    "f1.li2": "Garis konstruksi (<kbd>X</kbd>) untuk referensi tanpa mengganggu deteksi profil tertutup",
    "f1.li3": "Teks sketsa 2D — vektorisasi font TrueType/OpenType menjadi kurva siap ekstrusi",
    "f1.li4": "Snapping cerdas bertingkat: Endpoint → Midpoint → Center → Intersection → Grid",
    "f1.li5": "Constraint solver Levenberg-Marquardt: Coincident, Parallel, Perpendicular, Tangent, Symmetric, Equal, Distance, Radius, Angle",
    "f1.li6": "Trim, Extend, Offset paralel bi-arc, dan Mirror simetris",
    "f1.visualAlt": "Tampilan sketsa 2D DuCAD menunjukkan pembuatan poligon parametrik dengan panduan radius dan sudut",

    "f2.chip": "02 · Solid Modeling 3D",
    "f2.h3": "Pemodelan solid B-Rep 3D kelas industri",
    "f2.p": "Ditenagai kernel <strong>OpenCASCADE (OCCT)</strong> — kernel B-Rep open-source paling matang di industri — bukan sekadar mesh poligon, sehingga hasil model presisi dan siap manufaktur.",
    "f2.li1": "Extrude (Blind, Symmetric, Up to Face), Revolve sumbu 3D kustom, Loft multi-profil, Sweep sepanjang kurva",
    "f2.li2": "Geometri Helix/Spring/Coil untuk pegas, ulir baut, dan sudu auger",
    "f2.li3": "Boolean Union, Cut, dan Intersect",
    "f2.li4": "Fillet konstan &amp; variable-radius, Chamfer, Thin-Wall Shell, Draft Angle",
    "f2.li5": "Teks 3D Emboss/Deboss pada permukaan planar",
    "f2.li6": "Hole Wizard standar ISO: Simple, Counterbore, Countersink, Tapped (M2–M12)",
    "f2.visualAlt": "Tampilan aplikasi DuCAD saat menerapkan Hole Wizard ISO (Counterbore M6) dan bilah konteks pemodelan solid 3D",

    "f3.chip": "03 · Shell &amp; Direct Modeling",
    "f3.h3": "Thin-wall Shell &amp; operasi face intuitif",
    "f3.p": "Buat rongga pada objek solid dengan ketebalan dinding presisi dan lakukan operasi direct modeling push-pull pada bidang datar maupun lengkung secara real time.",
    "f3.li1": "<strong>Thin-Wall Shell / Hollow</strong> — membuat rongga bodi dengan ketebalan dinding yang dapat disesuaikan",
    "f3.li2": "<strong>Context bar pemilihan face</strong> — akses instan ke Sketsa di Face, Revolve, Helix, Rib, Draft Angle, dan Split Face",
    "f3.li3": "<strong>Datum Workplanes</strong> — bidang referensi Offset, Angled, dan 3-Point di koordinat 3D mana pun",
    "f3.li4": "Push-pull permukaan real-time dan deteksi cerdas boolean cut",
    "f3.visualAlt": "Tampilan aplikasi DuCAD saat menerapkan fitur Shell/Hollow pada solid 3D dengan pengaturan ketebalan dinding",

    "f4.chip": "04 · Parametric History",
    "f4.h3": "Riwayat parametrik berbasis graf dependensi",
    "f4.p": "Setiap langkah desain terekam sebagai node dalam <em>Directed Acyclic Graph</em> (DAG) yang dapat ditelusuri, diedit, dan diregenerasi — bukan sekadar daftar undo linear.",
    "f4.li1": "Feature Tree &amp; Riwayat lengkap dengan pencarian fitur cepat",
    "f4.li2": "Edit parameter fitur masa lalu → regenerasi otomatis seluruh geometri turunan",
    "f4.li3": "Riwayat tercatat per-operasi (Extrude, Cut Face, Shell, Push-Pull, dsb.) lengkap dengan cap waktu",
    "f4.visualAlt": "Panel Feature Tree DuCAD menampilkan riwayat parametrik: Cut Face, Tarik Sisi Solid, Shell Berlubang Sisi, Extrude Solid 3D",

    "f5.chip": "05 · Gambar Kerja 2D (ISO)",
    "f5.h3": "Gambar kerja teknik 2D siap produksi &amp; BOM",
    "f5.p": "Hasilkan gambar teknik standar multi-view dari model 3D lengkap dengan tata letak ISO/ANSI, arsir potongan melintang, dimensi otomatis, dan tabel Bill of Materials (BOM).",
    "f5.li1": "Format kertas standar: A0–A4 (Portrait &amp; Landscape) dengan border teknik dan title block ISO 5457",
    "f5.li2": "Proyeksi ortografis multi-view: Tampak Atas, Depan, Samping Kanan, dan pandangan Isometrik 3D",
    "f5.li3": "Section View (Tampak Potongan A-A) dengan arsir otomatis ISO/ANSI dan Hidden Line Removal",
    "f5.li4": "Detail View pembesaran dengan rasio skala fleksibel (2:1, 4:1, 5:1)",
    "f5.li5": "Tabel Bill of Materials (BOM) terintegrasi dan dimensi teknis asosiatif",
    "f5.li6": "Ekspor PDF &amp; SVG vektor resolusi tinggi siap cetak dan manufaktur",
    "f5.visualAlt": "Tampilan lembar gambar kerja teknik 2D DuCAD menunjukkan proyeksi multi-view, tampak potongan, detail view, tabel BOM, dan title block",

    "allFeat.eyebrow": "Cakupan Lengkap",
    "allFeat.h2": "8 modul, satu alur kerja terpadu",

    "card1.title": "Sketsa 2D Parametrik",
    "card1.desc": "Geometri presisi, constraint solver mandiri, dan snapping cerdas bertingkat untuk fondasi desain yang akurat.",
    "card2.title": "Solid Modeling B-Rep 3D",
    "card2.desc": "Extrude, Revolve, Loft, Sweep, Boolean, Fillet/Chamfer, Shell, Draft, dan Hole Wizard standar ISO — kelas industri via OCCT.",
    "card3.title": "Datum Workplanes",
    "card3.desc": "Bidang referensi bebas: Offset, Angled, dan 3-Point Plane di koordinat 3D mana pun.",
    "card4.title": "Gambar Kerja 2D (ISO)",
    "card4.desc": "Proyeksi multi-view, Hidden Line Removal, Section View berarsir ISO/ANSI, Detail View, dimensi otomatis, tabel BOM, dan title block.",
    "card5.title": "Assembly &amp; Clash Detection",
    "card5.desc": "Assembly Tree multi-part, 3D Mate Constraints (Concentric, Coincident, Distance, Angle), dan uji tabrakan otomatis.",
    "card6.title": "Parametric History Timeline",
    "card6.desc": "Riwayat desain berbasis DAG dengan regenerasi otomatis saat parameter fitur lama diubah.",
    "card7.title": "Interoperabilitas Format",
    "card7.desc": "Import/export STEP, DXF, GLTF/GLB, SVG, PDF, STL, OBJ, PLY, 3MF, dan format native <code>.ducad</code>.",
    "card8.title": "UI/UX Modern &amp; Studio Rendering",
    "card8.desc": "Command Palette, Radial Menu, ViewCube, PBR studio lighting dengan SSAO, dan dukungan 18+ bahasa.",
    "card9.title": "100% Offline &amp; Privat",
    "card9.desc": "Tanpa ketergantungan cloud — seluruh data desain tersimpan lokal di perangkat pengguna.",

    "why.eyebrow": "Mengapa DuCAD",
    "why.h2": "Kapabilitas kelas industri, tanpa jebakan biaya lisensi",
    "why.lead": "DuCAD dibangun sebagai alternatif modern terhadap CAD proprietary — mengadopsi kernel B-Rep sungguhan setara kelas industri, tanpa biaya lisensi jutaan dolar maupun langganan bulanan.",
    "why.thAspect": "Aspek",
    "why.thAutocad": "AutoCAD",
    "why.thShapr": "Shapr3D",
    "why.thDucad": "DuCAD",

    "why.row1.aspect": "Kernel solid modeling",
    "why.row1.autocad": "Autodesk ShapeManager (turunan ACIS)",
    "why.row1.shapr": "Siemens Parasolid (proprietary)",
    "why.row1.ducad": "OpenCASCADE (OCCT) — B-Rep sejati, open-source",

    "why.row2.aspect": "Biaya lisensi kernel",
    "why.row2.autocad": "Termasuk dalam lisensi mahal Autodesk",
    "why.row2.shapr": "Termasuk dalam langganan Shapr3D",
    "why.row2.ducad": "Tanpa biaya lisensi kernel",

    "why.row3.aspect": "Gambar kerja 2D (Drawing)",
    "why.row3.autocad": "Standar industri kuat, termasuk dalam paket",
    "why.row3.shapr": "Add-on berbayar terpisah (≈$38/bulan)",
    "why.row3.ducad": "Termasuk penuh, tanpa biaya tambahan",

    "why.row4.aspect": "Penyimpanan &amp; privasi data",
    "why.row4.autocad": "Proprietary (.dwg) + Autodesk Cloud",
    "why.row4.shapr": "Proprietary (.shapr) + sinkronisasi cloud",
    "why.row4.ducad": "Format terbuka (.ducad, STEP, DXF) — 100% offline",

    "why.row5.aspect": "Stack &amp; performa runtime",
    "why.row5.autocad": "C++ monolitik puluhan tahun, RAM besar (~1.5–3GB)",
    "why.row5.shapr": "Swift + C++ (Parasolid), macOS/iOS-sentris",
    "why.row5.ducad": "100% Rust modern, memory-safe, RAM &lt;150MB, startup &lt;1 detik",

    "why.row6.aspect": "Direct modeling (push/pull)",
    "why.row6.autocad": "Perintah terpisah (PRESSPULL/EXTRUDE/SUBTRACT)",
    "why.row6.shapr": "Adaptive Push/Pull otomatis",
    "why.row6.ducad": "Smart Boolean Cut Detection — deteksi tumpang tindih real-time",

    "why.note": "Ringkasan diadaptasi dari analisis komparatif teknis internal DuCAD terhadap arsitektur AutoCAD dan Shapr3D.",

    "interop.eyebrow": "Interoperabilitas",
    "interop.h2": "Terbuka untuk seluruh alur kerja manufaktur &amp; web",
    "interop.importTitle": "Import",
    "interop.import1": "<strong>STEP</strong> <span>(.step, .stp)</span> — model CAD standar internasional B-Rep",
    "interop.import2": "<strong>DXF</strong> <span>(.dxf)</span> — sketsa vektor 2D AutoCAD R12/2000+",
    "interop.import3": "<strong>.ducad</strong> <span>native</span> — dokumen JSON berisi geometri B-Rep, sketsa &amp; riwayat",
    "interop.exportTitle": "Export",
    "interop.export1": "<strong>STEP</strong> <span>(.step, .stp)</span> — solid B-Rep penuh untuk CNC/CAM",
    "interop.export2": "<strong>GLTF / GLB</strong> <span>(.glb)</span> — 3D Web &amp; AR Quick Look (iOS/Android) dengan material PBR",
    "interop.export3": "<strong>SVG</strong> <span>(.svg)</span> — vektor 2D untuk Laser Cutting &amp; CNC Router",
    "interop.export4": "<strong>PDF</strong> <span>(.pdf)</span> — gambar kerja teknik vektor ISO resolusi tinggi",
    "interop.export5": "<strong>STL, OBJ, PLY, 3MF</strong> — mesh untuk 3D Printing / Slicer",

    "workflow.eyebrow": "Alur Kerja",
    "workflow.h2": "Dirancang untuk kecepatan tangan dan mata",
    "wf1.title": "Command Palette",
    "wf1.desc": "Akses instan ke seluruh tool via pencarian teks.",
    "wf2.title": "Radial Menu",
    "wf2.desc": "Menu melingkar di bawah kursor untuk tool esensial.",
    "wf3.title": "ViewCube 3D",
    "wf3.desc": "Kontrol orientasi kamera: Top, Front, Right, Isometric, Orbit.",
    "wf4.title": "Studio Lighting &amp; PBR",
    "wf4.desc": "Preset Warm Studio, Cool Tech, High Contrast, Sunset Gold, dan Cyberpunk Neon dengan SSAO.",

    "arch.eyebrow": "Di Balik Layar",
    "arch.h2": "Arsitektur workspace multi-crate",
    "arch.lead": "Dibangun modular dalam 8 crate Rust — setiap lapisan terisolasi dengan tanggung jawab jelas.",
    "crate1.desc": "Model dokumen, undo/redo, pohon perakitan, mate, unit",
    "crate2.desc": "Mesin sketsa 2D, constraint solver, snapping, region solver",
    "crate3.desc": "Pembungkus B-Rep OpenCASCADE: boolean, fillet, hole, helix, section",
    "crate4.desc": "Engine wgpu: kamera 3D, shader PBR, SSAO, grid, overlay sketsa",
    "crate5.desc": "Import/export STEP, GLB/GLTF, SVG, PDF, DXF, STL, OBJ",
    "crate6.desc": "Komponen egui: toolbar, context bar, HUD, drawing sheet, drawers",
    "crate7.desc": "Lokalisasi &amp; kamus terjemahan 18+ bahasa",
    "crate8.desc": "Aplikasi utama, event loop winit/eframe, manajemen window",

    "mulai.eyebrow": "Mulai Sekarang",
    "mulai.h2": "Jalankan DuCAD dari source dalam hitungan menit",
    "mulai.li1": "Rust Toolchain terbaru (1.75+ stabil) via <code>rustup</code>",
    "mulai.li2": "CMake ≥ 3.16 dan C++17 compiler (Clang/GCC/MSVC) untuk membangun kernel OCCT",
    "mulai.li3": "macOS (Apple Silicon &amp; Intel), Linux (X11/Wayland), atau Windows 10/11",
    "mulai.note": "Kompilasi pertama membangun kernel OpenCASCADE dari source (~8–15 menit) dan di-cache permanen di <code>target/</code> — build berikutnya instan.",
    "mulai.copyBtn": "Salin",
    "mulai.copiedBtn": "Tersalin ✓",
    "mulai.codeComment1": "# Jalankan aplikasi",
    "mulai.codeComment2": "# Jalankan unit &amp; integration test",

    "footer.tagline": "Design Universe CAD — CAD 2D/3D modern, parametrik, dan berkinerja tinggi, ditulis murni dalam Rust.",
    "footer.navHeader": "Navigasi",
    "footer.projectHeader": "Proyek",
    "footer.repo": "Repositori GitHub",
    "footer.licenseMit": "Lisensi MIT",
    "footer.licenseApache": "Lisensi Apache-2.0",
    "footer.copyright": "&copy; 2026 DuCAD. Seluruh hak cipta dihormati sesuai lisensi dual MIT/Apache-2.0.",
    "footer.credit": "Dibuat oleh",
    "footer.toTopAria": "Kembali ke atas",

    "_meta.title": "DuCAD — CAD 2D/3D Modern, Parametrik & Berkinerja Tinggi",
    "_meta.description": "DuCAD adalah software CAD 2D/3D parametrik berkinerja tinggi berbasis Rust, menggabungkan presisi drafting AutoCAD dengan kemudahan direct modeling Shapr3D, didukung kernel solid modeling industri OpenCASCADE (OCCT) dan rendering WebGPU."
  };

  /* Runtime-only strings that aren't tied to a persistent [data-i18n]
     element (e.g. the transient "Copied" button state). */
  const extraEn = {
    "mulai.copiedBtn": "Copied ✓"
  };

  /* ---------- Capture original English content from the DOM ---------- */
  const originalHTML = {};
  const originalAlt = {};
  const originalAria = {};
  let originalTitle = document.title;
  let originalDescription = "";

  function captureOriginals() {
    document.querySelectorAll("[data-i18n]").forEach((el) => {
      const key = el.getAttribute("data-i18n");
      if (!(key in originalHTML)) originalHTML[key] = el.innerHTML;
    });
    document.querySelectorAll("[data-i18n-alt]").forEach((el) => {
      const key = el.getAttribute("data-i18n-alt");
      if (!(key in originalAlt)) originalAlt[key] = el.getAttribute("alt") || "";
    });
    document.querySelectorAll("[data-i18n-aria]").forEach((el) => {
      const key = el.getAttribute("data-i18n-aria");
      if (!(key in originalAria)) originalAria[key] = el.getAttribute("aria-label") || "";
    });
    const descEl = document.getElementById("pageDescription");
    originalDescription = descEl ? descEl.getAttribute("content") || "" : "";
  }

  let currentLang = DEFAULT_LANG;

  function resolve(key) {
    if (currentLang === "en") {
      return extraEn[key] ?? originalHTML[key] ?? null;
    }
    return id[key] ?? extraEn[key] ?? originalHTML[key] ?? null;
  }

  function applyLang(lang) {
    currentLang = lang === "id" ? "id" : "en";

    document.querySelectorAll("[data-i18n]").forEach((el) => {
      const key = el.getAttribute("data-i18n");
      const value = currentLang === "en" ? originalHTML[key] : (id[key] ?? originalHTML[key]);
      if (value != null) el.innerHTML = value;
    });

    document.querySelectorAll("[data-i18n-alt]").forEach((el) => {
      const key = el.getAttribute("data-i18n-alt");
      const value = currentLang === "en" ? originalAlt[key] : (id[key] ?? originalAlt[key]);
      if (value != null) el.setAttribute("alt", value);
    });

    document.querySelectorAll("[data-i18n-aria]").forEach((el) => {
      const key = el.getAttribute("data-i18n-aria");
      const value = currentLang === "en" ? originalAria[key] : (id[key] ?? originalAria[key]);
      if (value != null) el.setAttribute("aria-label", value);
    });

    const titleEl = document.getElementById("pageTitle");
    if (titleEl) {
      document.title = currentLang === "en" ? originalTitle : (id["_meta.title"] || originalTitle);
    }
    const descEl = document.getElementById("pageDescription");
    if (descEl) {
      descEl.setAttribute(
        "content",
        currentLang === "en" ? originalDescription : (id["_meta.description"] || originalDescription)
      );
    }

    document.documentElement.setAttribute("lang", currentLang);

    document.querySelectorAll("[data-lang-btn]").forEach((btn) => {
      btn.classList.toggle("is-active", btn.getAttribute("data-lang-btn") === currentLang);
    });

    try {
      localStorage.setItem(STORAGE_KEY, currentLang);
    } catch (err) {
      // Storage unavailable (private mode / blocked) — language choice just
      // won't persist across reloads; the page still works fine.
    }
  }

  function initialLang() {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved === "en" || saved === "id") return saved;
    } catch (err) {
      // Storage unavailable — fall through to default.
    }
    return DEFAULT_LANG;
  }

  function init() {
    captureOriginals();
    applyLang(initialLang());

    document.querySelectorAll("[data-lang-btn]").forEach((btn) => {
      btn.addEventListener("click", () => applyLang(btn.getAttribute("data-lang-btn")));
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }

  return {
    getLang: () => currentLang,
    setLang: applyLang,
    t: resolve
  };
})();
