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
    "nav.agent": "Agent",
    "nav.mengapa": "Mengapa DuCAD",
    "nav.arsitektur": "Arsitektur",
    "nav.mulai": "Mulai",
    "nav.github": "GitHub",
    "nav.download": "Unduh",
    "nav.ctaDownload": "Unduh",
    "nav.toggleAria": "Buka menu navigasi",

    "tag.new": "Baru",
    "hero.pill": "Baru di 0.2 — simulasi, sheet metal, dan agent AI di dalam aplikasi",
    "hero.h1": "Gambar. Hitung. <span class=\"text-gradient\">Produksi.</span>",
    "hero.lead": "DuCAD adalah CAD 2D/3D parametrik di atas kernel B-rep industri <strong>OpenCASCADE</strong>, ditulis dengan <strong>Rust</strong>. Modelkan part, jalankan studi tegangan, terbitkan gambar kerja ISO — atau serahkan seluruh pekerjaan ke agent AI dan saksikan ia bekerja di viewport Anda.",
    "hero.ctaPrimary": "Unduh untuk macOS, Windows &amp; Linux",
    "hero.ctaSecondary": "Lihat di GitHub",
    "hero.visualAlt": "Viewport DuCAD menampilkan heatmap tegangan von Mises pada rumah bearing pillow block setelah studi statik",
    "proof.bodies": "body, 1,27 kg",
    "proof.stress": "von Mises puncak",
    "proof.sf": "faktor keamanan",
    "proof.checks": "check desain lulus",
    "hero.note": "Setiap screenshot di halaman ini adalah aplikasi yang sedang berjalan. Part ini dimodelkan oleh agent lewat jembatan live DuCAD sendiri — 36 operasi yang bisa di-replay.",

    "stats.label1": "operasi pemodelan",
    "stats.label2": "jenis studi simulasi",
    "stats.label3": "tool MCP untuk agent",
    "stats.label4": "material teknik",
    "stats.label5": "aplikasi native: macOS dan iPad",

    "featHead.eyebrow": "Satu alur kerja",
    "featHead.h2": "Dari sketsa pertama sampai part yang bisa dipercaya",
    "featHead.lead": "Geometri, analisis, dokumentasi, dan otomasi berbagi satu dokumen — jadi satu dimensi yang berubah mengalir ke semuanya.",

    "f1.chip": "Pemodelan solid",
    "f1.h3": "Solid B-rep sungguhan, dibangun dari parameter",
    "f1.p": "Bukan mesh poligon: permukaan eksak dari kernel <strong>OpenCASCADE</strong>. Setiap dimensi bisa menjadi parameter bernama, sehingga satu perubahan membangun ulang seluruh part.",
    "f1.li1": "Extrude, revolve, loft, sweep, helix, boolean, pattern, mirror, split",
    "f1.li2": "Fillet (konstan atau variabel), chamfer, draft, dan shell yang bisa membuka face mana pun sampai kedalaman tertentu",
    "f1.li3": "Hole wizard ISO M2–M12: clearance, tapped, counterbore, countersink",
    "f1.li4": "Baut, mur, ring, dan bearing ISO dari toolbox, dengan ulir kosmetik atau ulir potong",
    "f1.li5": "Material PBR dan pencahayaan studio, dengan SSAO dan bayangan kontak",
    "f1.visualAlt": "Rakitan pillow block di viewport 3D DuCAD dengan material aluminium dan krom",

    "f2.chip": "Simulasi",
    "f2.h3": "Ketahui kekuatannya sebelum memotong logam",
    "f2.p": "Solver elemen hingga sudah ada di dalam aplikasi. Jepit satu face, beri beban, tekan jalankan — medan tegangan langsung tergambar di model Anda.",
    "f2.li1": "Tegangan statik linier, frekuensi alami, buckling linier, panas tunak, dan tegangan termal",
    "f2.li2": "Beban: gaya, tekanan, torsi, gravitasi, beban bearing, dan beban jarak jauh",
    "f2.li3": "Mesh hex voxel secara bawaan, tetrahedra kuadratik untuk part melengkung",
    "f2.li4": "Massa, pusat massa, dan tensor inersia per body maupun per rakitan",
    "f2.fine": "Hasil adalah estimasi teknik (sekitar ±10 % pada mesh bawaan), bukan angka sertifikasi.",
    "f2.visualAlt": "Heatmap tegangan von Mises pada rumah bearing setelah studi statik di panel Simulasi DuCAD",

    "f3.chip": "Agent AI",
    "f3.h3": "Agent yang memodelkan, mengukur, dan menjawab",
    "f3.p": "Buka sidebar Chat AI dan minta dengan bahasa sehari-hari. Agent memakai tool yang sama dengan Anda — memeriksa geometri, menjalankan check, dan mengubah model di depan mata Anda.",
    "f3.li1": "Bekerja dengan Claude, provider yang kompatibel dengan OpenAI, atau agent CLI lokal seperti Claude Code",
    "f3.li2": "Satu batch agent adalah satu langkah undo; perubahan berisiko datang sebagai pratinjau hijau/merah yang Anda terima atau tolak",
    "f3.li3": "Operasi yang gagal kembali dengan penyebab yang spesifik dan perbaikan yang sudah diverifikasi lewat dry run",
    "f3.li4": "Bawaan hanya di perangkat — AI eksternal baru aktif bila Anda mengizinkannya",
    "f3.visualAlt": "Sidebar Chat AI DuCAD: agent memanggil Inspect dan Run checks, lalu melaporkan massa rumah bearing dan kelima check lulus",

    "f4.chip": "Gambar kerja",
    "f4.h3": "Gambar manufaktur, dibangkitkan dari model",
    "f4.p": "Satu klik mengubah solid menjadi lembar ISO: tampak proyeksi, potongan, dimensi, bill of materials, dan balon bernomor.",
    "f4.li1": "Lembar A0–A4 dengan bingkai dan kepala gambar ISO 5457",
    "f4.li2": "Tampak atas, depan, samping, isometrik, potongan, dan detail dengan penghilangan garis tersembunyi",
    "f4.li3": "Suaian ISO 286 dan anotasi GD&amp;T, plus check tolerance stack-up",
    "f4.li4": "Ekspor vektor PDF, SVG, dan DXF",
    "f4.visualAlt": "Lembar gambar DuCAD untuk pillow block dengan tampak atas, depan, isometrik, dan potongan, dimensi, balon, serta bill of materials",

    "allFeat.eyebrow": "Semua sudah termasuk",
    "allFeat.h2": "Isi toolbox selebihnya",
    "card1.title": "Sheet metal",
    "card1.desc": "Base flange, edge flange, hem, dan jog dengan radius tekuk dan k-factor. Bentangkan menjadi pola datar dan ekspor DXF dengan garis tekuk di layer terpisah.",
    "card2.title": "Konfigurasi",
    "card2.desc": "Varian dari satu desain: penimpaan parameter, fitur yang disuppress, penggantian material, design table CSV.",
    "card3.title": "Toleransi &amp; GD&amp;T",
    "card3.desc": "Suaian ISO 286, simbol GD&amp;T di lembar gambar, stack-up worst-case atau RSS.",
    "card4.title": "Part standar",
    "card4.desc": "Baut ISO 4762 dan 4014, mur, ring, pin, dan ball bearing, tercantum di BOM dengan nomor standarnya.",
    "card5.title": "Check desain",
    "card5.desc": "Persyaratan ditulis sebagai data — massa, tebal dinding, clearance, tegangan puncak — dievaluasi ulang setelah setiap perubahan.",
    "card6.title": "Sketsa ber-constraint",
    "card6.desc": "Garis, busur, elips, poligon, slot, spline, dan teks, dijaga solver constraint geometri dan dimensi dengan snapping bertingkat.",
    "card7.title": "Input vektor dan pensil",
    "card7.desc": "Gambar path Bézier dengan fill dan layer, atau sketsa bebas dengan Apple Pencil — coretan dikenali sebagai garis, busur, dan lingkaran, diberi constraint, lalu diekstrusi langsung ke 3D dengan warnanya sendiri.",
    "card8.title": "Rakitan",
    "card8.desc": "Mate, cek interferensi, kopling roda gigi, sekrup, dan rack, serta tampak urai berurutan.",
    "card9.title": "Riwayat parametrik",
    "card9.desc": "Feature tree yang bisa dicari. Ubah langkah awal dan semua yang sesudahnya dibangun ulang.",
    "card10.title": "Format terbuka",
    "card10.desc": "STEP masuk dan keluar. Ekspor STL, OBJ, GLB, SVG, PDF, dan DXF. Berkas native <code>.ducad</code> adalah JSON biasa.",

    "agent.eyebrow": "Desain sebagai kode",
    "agent.h2": "Mesin yang sama, tanpa jendela",
    "agent.lead": "Setiap langkah pemodelan adalah operasi JSON dalam log yang bisa di-replay. Karena itu sebuah part bisa di-diff, di-review, diuji di CI, dan diserahkan ke agent.",
    "agent.li1": "Run, inspect, check, render, simulasi, dan build dari terminal.",
    "agent.li2t": "Server MCP",
    "agent.li2": "27 tool lewat stdio. Dengan <code>--attach</code>, agent mengendalikan aplikasi yang sedang terbuka dan Anda melihat setiap langkahnya.",
    "agent.li3t": "Ramah Git",
    "agent.li3": "Satu operasi per baris, plus diff geometri yang menunjukkan volume yang bertambah dan berkurang.",
    "agent.li4t": "CI untuk hardware",
    "agent.li4": "Build deterministik menulis STEP, STL, PDF, PNG, dan BOM. Check yang gagal menghentikannya.",
    "agent.fine": "Cuplikan dari model yang ditampilkan di atas; keluaran terminal diringkas dari laporan build.",

    "why.eyebrow": "Mengapa DuCAD",
    "why.h2": "Dibuat untuk dimiliki, bukan disewa",
    "why1.title": "Kernel terbuka",
    "why1.desc": "OpenCASCADE adalah kernel B-rep open-source yang matang. Tanpa lisensi kernel, tanpa biaya per kursi.",
    "why2.title": "Berkas yang bisa dibaca",
    "why2.desc": "Desain berupa JSON dan STEP. Tidak ada yang terkunci di wadah proprietary.",
    "why3.title": "Privat secara bawaan",
    "why3.desc": "Geometri Anda tetap di perangkat Anda. AI jaringan dan jembatan agent mati sampai Anda menyalakannya.",
    "why4.title": "Perangkat lunak bebas",
    "why4.desc": "Rust dari solver sketsa sampai renderer, dirilis di bawah AGPL-3.0.",
    "arch.label": "16 crate Rust, satu arah dependensi",

    "download.eyebrow": "Unduh",
    "download.h2": "Dapatkan DuCAD untuk desktop Anda",
    "download.lead": "Binary siap pakai untuk setiap rilis. Pilih platform Anda — setiap tautan membuka rilis GitHub terbaru, tempat installer untuk sistem tersebut dilampirkan.",
    "download.macDesc": "Apple Silicon (arm64). Ambil build macOS dari rilis terbaru.",
    "download.macBtn": "Unduh untuk macOS",
    "download.winDesc": "Windows 10 dan 11, 64-bit. Ambil build Windows dari rilis terbaru.",
    "download.winBtn": "Unduh untuk Windows",
    "download.linuxDesc": "Distribusi x86-64. Ambil build Linux dari rilis terbaru.",
    "download.linuxBtn": "Unduh untuk Linux",
    "download.note": "Semua unduhan dihosting di halaman <a href=\"https://github.com/cmjcode/ducad/releases\" target=\"_blank\" rel=\"noopener\">GitHub Releases</a>. Lebih suka membangun sendiri? Lihat <a href=\"#mulai\">Mulai</a> di bawah.",

    "mulai.eyebrow": "Mulai",
    "mulai.h2": "Bangun dari source",
    "mulai.li1": "Toolchain Rust stabil lewat <code>rustup</code>",
    "mulai.li2": "CMake 3.16 atau lebih baru dan compiler C++17, untuk membangun kernel OpenCASCADE",
    "mulai.li3": "macOS Apple Silicon dan iPadOS adalah target utama; Linux dibangun dan diuji di CI",
    "mulai.note": "Build pertama mengompilasi OpenCASCADE (sekitar 8–15 menit) dan menyimpannya di <code>target/</code>. Build berikutnya cepat.",
    "mulai.copyBtn": "Salin",
    "mulai.copiedBtn": "Tersalin",
    "mulai.codeComment1": "# Jalankan aplikasi",
    "mulai.codeComment2": "# Pasang CLI dan server MCP untuk agent",

    "footer.tagline": "Design Universe CAD — CAD 2D/3D parametrik dengan simulasi dan agent AI, ditulis dengan Rust.",
    "footer.navHeader": "Navigasi",
    "footer.projectHeader": "Proyek",
    "footer.repo": "Repositori GitHub",
    "footer.privacyPolicy": "Kebijakan Privasi",
    "footer.license": "Lisensi AGPL-3.0",
    "footer.copyright": "&copy; 2026 DuCAD. Dirilis di bawah lisensi AGPL-3.0.",
    "footer.credit": "Dibuat oleh",
    "footer.toTopAria": "Kembali ke atas",

    "privacy.backHome": "← Kembali ke Beranda",
    "privacy.badge": "Standar Kepatuhan Apple App Store &amp; Perlindungan Data",
    "privacy.title": "Kebijakan Privasi DuCAD",
    "privacy.effective": "Berlaku Sejak: 1 Januari 2026",
    "privacy.updated": "Terakhir Diperbarui: September 2026",
    "privacy.platform": "Platform: macOS, iPadOS, iOS, Windows, Linux",
    "privacy.summaryTitle": "Ringkasan Privasi Inti (Zero Data Collection)",
    "privacy.sum1": "<strong>100% Offline &amp; Local-First:</strong> Semua model 3D, sketsa 2D, riwayat parametrik, dan gambar kerja disimpan di perangkat Anda sendiri.",
    "privacy.sum2": "<strong>Nol Pengumpulan Data Pribadi:</strong> Kami tidak mengumpulkan nama, email, nomor telepon, alamat IP, ataupun lokasi Anda.",
    "privacy.sum3": "<strong>Tanpa Pelacakan &amp; Tanpa Iklan:</strong> Tidak ada SDK analitik pihak ketiga, pelacak perilaku, maupun jaringan iklan di dalam aplikasi.",
    "privacy.sum4": "<strong>Tanpa Akun:</strong> DuCAD langsung dapat digunakan tanpa registrasi, tanpa login, dan tanpa cloud pihak ketiga wajib.",
    "privacy.sum5": "<strong>Kontrol Penuh:</strong> Anda memiliki kendali 100% atas file dan data proyek Anda sendiri.",
    "privacy.sec1Title": "1. Pendahuluan",
    "privacy.sec1P1": "DuCAD (\"kami\"), yang dikembangkan oleh PT. VNEU TEKNOLOGI INDONESIA, berkomitmen penuh untuk menghormati dan melindungi privasi Anda. Kebijakan Privasi ini menjelaskan bagaimana informasi ditangani saat Anda menggunakan aplikasi DuCAD pada seluruh platform yang didukung, termasuk macOS dan iPadOS di Apple App Store.",
    "privacy.sec1P2": "Prinsip utama kami sangat sederhana: <strong>DuCAD dirancang dengan arsitektur lokal murni (offline-first). Kami tidak mengumpulkan, menyimpan di server kami, memproses, ataupun menjual data pribadi Anda.</strong>",
    "privacy.sec2Title": "2. Informasi yang TIDAK Kami Kumpulkan",
    "privacy.sec2P": "Berbeda dengan layanan berbasis cloud pada umumnya, DuCAD beroperasi sepenuhnya di perangkat lokal Anda. Secara spesifik:",
    "privacy.sec2Li1": "<strong>Data Identitas Pribadi:</strong> Kami tidak meminta atau mengumpulkan nama, alamat, alamat email, nomor telepon, atau data kependudukan.",
    "privacy.sec2Li2": "<strong>Akun Pengguna:</strong> Tidak diperlukan pembuatan akun atau kata sandi untuk menggunakan DuCAD.",
    "privacy.sec2Li3": "<strong>Telemetri &amp; Analitik Penggunaan:</strong> Kami tidak melacak fitur apa yang Anda klik, seberapa sering Anda membuka aplikasi, atau berapa lama durasi pemodelan Anda.",
    "privacy.sec2Li4": "<strong>Informasi Perangkat Sensitif:</strong> Kami tidak mengumpulkan nomor seri perangkat, IDFA (Identifier for Advertisers), atau data biometrik.",
    "privacy.sec2Li5": "<strong>Pelacak &amp; Iklan Pihak Ketiga:</strong> DuCAD bebas dari SDK iklan, pelacak pihak ketiga (seperti Facebook SDK, Google Analytics, Firebase), dan broker data.",
    "privacy.sec3Title": "3. Penyimpanan File &amp; Desain Proyek Anda",
    "privacy.sec3P1": "Semua berkas desain CAD (sketsa 2D, pemodelan solid 3D, assembly, gambar kerja teknik ISO, dan berkas ekspor seperti STEP, STL, OBJ, DXF, SVG, PDF, GLTF) dibuat, dihitung oleh kernel OpenCASCADE, dan disimpan <strong>sepenuhnya di penyimpanan lokal perangkat Anda</strong>.",
    "privacy.sec3P2": "Kami tidak memiliki akses ke konten proyek, hak kekayaan intelektual, atau geometri desain Anda. Data Anda tidak pernah dikirimkan ke server DuCAD ataupun pihak ketiga tanpa tindakan eksplisit dari Anda.",
    "privacy.sec4Title": "4. iCloud &amp; Penyimpanan yang Dikelola Pengguna",
    "privacy.sec4P": "Pada macOS dan iPadOS, Anda dapat secara opsional memilih untuk menyimpan atau membuka file proyek di Apple iCloud Drive atau penyedia penyimpanan dokumen lainnya. Apabila Anda menggunakan iCloud, sinkronisasi dan keamanan data dikelola langsung oleh Apple sesuai Kebijakan Privasi Apple. DuCAD tidak mengoperasikan server perantara dan tidak memiliki akses ke akun Apple ID Anda.",
    "privacy.sec5Title": "5. Izin Perangkat (Device Permissions)",
    "privacy.sec5P": "DuCAD dirancang mengikuti prinsip hak akses minimal (least-privilege) dan perlindungan Sandbox Apple:",
    "privacy.sec5Li1": "<strong>Akses File &amp; Dokumen:</strong> Digunakan hanya saat Anda membuka, menyimpan, atau mengekspor berkas melalui antarmuka pemilih file sistem standar (Document Picker). DuCAD hanya memiliki akses ke file atau direktori yang Anda pilih.",
    "privacy.sec5Li2": "<strong>Akselerasi Grafis (GPU / WebGPU / Metal):</strong> Digunakan murni di dalam perangkat untuk merender visual 3D dan antarmuka dengan performa tinggi. Tidak ada data frame visual yang direkam atau dikirimkan keluar.",
    "privacy.sec5Li3": "<strong>Tanpa Akses Kamera, Mikrofon, Kontak, atau Lokasi:</strong> DuCAD tidak pernah meminta atau mengakses kamera, mikrofon, daftar kontak, atau lokasi geografis presisi Anda.",
    "privacy.sec6Title": "6. Layanan Pihak Ketiga &amp; Tautan Eksternal",
    "privacy.sec6P": "DuCAD tidak menggunakan layanan analitik atau pelacak pihak ketiga di dalam aplikasi. Jika Anda mengeklik tautan eksternal (misalnya ke repositori GitHub kami di github.com/cmjcode/ducad atau lisensi open-source), peramban bawaan Anda akan membuka situs terkait yang tunduk pada kebijakan privasi masing-masing situs tersebut.",
    "privacy.sec7Title": "7. Privasi Anak-Anak (COPPA)",
    "privacy.sec7P": "DuCAD tidak mengumpulkan data pribadi dari siapa pun, termasuk anak-anak di bawah usia 13 tahun (atau batas usia yang berlaku di yurisdiksi Anda). Aplikasi ini aman digunakan untuk pendidikan, sekolah teknik, mahasiswa, dan desainer segala usia.",
    "privacy.sec8Title": "8. Retensi &amp; Penghapusan Data",
    "privacy.sec8P": "Karena kami tidak mengumpulkan atau menyimpan data Anda di server kami, kami tidak menyimpan data pribadi Anda. Anda memegang kendali penuh atas semua file di perangkat Anda: menghapus file proyek dari perangkat Anda atau dari iCloud akan menghapusnya secara permanen seketika.",
    "privacy.sec9Title": "9. Hak Pengguna (GDPR, CCPA/CPRA, &amp; UU PDP)",
    "privacy.sec9P": "Berdasarkan peraturan perlindungan data global (seperti GDPR di Uni Eropa, CCPA/CPRA di California, dan UU Pelindungan Data Pribadi di Indonesia), pengguna memiliki hak untuk mengakses, memperbaiki, menghapus, atau membatasi pemrosesan data pribadi mereka. Mengingat DuCAD tidak mengumpulkan atau memproses data pribadi apa pun, privasi Anda terlindungi secara bawaan (privacy by design).",
    "privacy.sec10Title": "10. Perubahan pada Kebijakan Privasi Ini",
    "privacy.sec10P": "Kami dapat memperbarui Kebijakan Privasi ini sewaktu-waktu untuk menyesuaikan dengan pembaruan aplikasi atau regulasi terbaru. Versi terbaru akan selalu dipublikasikan di halaman ini (https://ducad.app/PrivacyPolicy.html) dengan mencantumkan tanggal revisi terbaru.",
    "privacy.sec11Title": "11. Hubungi Kami",
    "privacy.sec11P": "Jika Anda memiliki pertanyaan mengenai Kebijakan Privasi ini atau praktik privasi kami, silakan hubungi kami melalui:",

    "_meta.title": "DuCAD — CAD parametrik dengan simulasi dan agent AI",
    "_meta.description": "DuCAD adalah aplikasi CAD 2D/3D parametrik berbasis Rust di atas kernel OpenCASCADE: pemodelan solid B-rep, simulasi struktur dan termal, sheet metal, gambar kerja ISO dengan GD&T, dan agent AI yang memodelkan di dalam aplikasi."
  };

  /* Runtime-only strings that aren't tied to a persistent [data-i18n]
     element (e.g. the transient "Copied" button state). */
  const extraEn = {
    "mulai.copiedBtn": "Copied"
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
