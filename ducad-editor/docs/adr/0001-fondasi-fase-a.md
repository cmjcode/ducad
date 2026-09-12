# ADR 0001 — Keputusan Fondasi Fase A (P0)

**Status**: Diterima · **Tanggal**: 2026-09-12 · **Cakupan**: P0.2, P0.6

Fase A menyiapkan fondasi sebelum DuCAD bisa dikembangkan ke level AutoCAD/
SolidWorks/Shapr3D. Dokumen ini mencatat keputusan yang diambil beserta
alasan dan alternatif yang ditolak, supaya tidak perlu diperdebatkan ulang.

---

## 1. Fork OCCT dijadikan submodule yang dideklarasikan

**Masalah.** `ducad-editor/vendors/opencascade-rs` sudah lama tercatat di
git sebagai gitlink (mode `160000`) tetapi **tanpa `.gitmodules`**.
Akibatnya `git clone` biasa menghasilkan direktori kosong, dan karena
`[patch.crates-io]` di manifes workspace menunjuk ke dalamnya, `cargo`
gagal saat **resolusi dependensi** — sebelum satu baris pun dikompilasi.
Repo praktis tidak dapat dibangun ulang oleh siapa pun selain mesin asal,
dan CI mustahil dijalankan.

Ditemukan juga satu tambalan penting pada `crates/opencascade-sys/build.rs`
yang **belum di-commit sama sekali**: pengemasan ulang arsip statis Apple
lewat `libtool` (menghindari pemotongan argumen `ar` karena ARG_MAX),
penautan seluruh `libTK*.a`, dan `libc++`. Tanpa itu tautan statis Apple
gagal. Tambalan ini hanya ada di disk satu mesin.

**Keputusan.** Menambahkan `.gitmodules` yang mendeklarasikan submodule
(URL **HTTPS**, bukan SSH, karena runner CI tidak punya kunci SSH), serta
meng-commit tambalan `build.rs` ke fork `ducad-patches`.

**Alternatif yang ditolak.**
- *Menyalin OCCT apa adanya ke dalam repo (vendoring penuh).* Repo
  membengkak puluhan ribu berkas dan tambalan kita jadi sulit dibedakan
  dari kode upstream.
- *Menjadikannya crate terpisah di registry.* Tambalan masih sering
  berubah; siklus rilis akan memperlambat pengembangan kernel.

**Konsekuensi.** Clone **wajib** memakai `--recurse-submodules` (README
sudah diperbaiki). Commit fork harus di-*push* sebelum CI bisa hijau.

---

## 2. Gate `rustfmt` dipasang sebagai informasi dulu, belum memblokir

**Masalah.** Basis kode belum pernah dilewatkan rustfmt:
`cargo fmt --all -- --check` melaporkan ~1.628 selisih yang tersebar di
hampir seluruh berkas.

**Keputusan.** Job `fmt` berjalan di CI dengan `continue-on-error: true`.
Cara membereskannya didokumentasikan langsung di `ci.yml` sebagai prosedur
satu kali; gate dibuat memblokir setelah itu.

**Alternatif yang ditolak.**
- *Reformat massal sekarang juga.* Secara teknis benar (rustfmt menjaga
  semantik, dan ada 361 test untuk membuktikannya), tetapi menyentuh tiap
  berkas dan akan bentrok keras dengan pekerjaan yang sedang berjalan di
  branch lain — branch aktif saat ini punya sinkronisasi upstream yang
  tertunda. Informasi soal branch lain ada di tangan pemilik repo, bukan
  di repo, jadi keputusannya diserahkan ke sana.
- *Memberlakukan hanya pada berkas yang disentuh PR.* **Dicoba lalu
  ditolak berdasarkan bukti**: berkas di sini besar-besar (`app.rs` 3.836
  baris, `model.rs` 1.822 baris), sehingga menambah satu alias tipe saja
  memaksa reformat seluruh berkas — persis churn yang ingin dihindari.
  Diverifikasi: 9 berkas yang disentuh Fase A menghasilkan 434 selisih
  rustfmt, padahal perubahan nyatanya cuma belasan baris.
- *Tidak memakai gate sama sekali.* Selisih gaya akan terus tumbuh tanpa
  ada yang mengukur.

**Konfigurasi.** Memakai **default rustfmt** (`max_width = 100`), tanpa
`rustfmt.toml`. Diukur secara empiris: default menghasilkan selisih
terkecil (1.628) dibanding `use_small_heuristics="Max"` (1.806),
`max_width=110` (1.685), atau gabungan keduanya (1.986). Persentil panjang
baris kode saat ini p95 = 91, p99 = 113 — cocok dengan default.

## 3. `clippy -D warnings` LANGSUNG memblokir; `cargo-deny` belum

**Temuan.** Seluruh workspace hanya menghasilkan **6 peringatan clippy
unik** — jauh lebih bersih dari dugaan. Keenamnya diperbaiki di tempat
(alias tipe untuk tipe kembalian kompleks, `Box` untuk varian enum besar,
`if` bersarang, `len() > 0`, dan satu variabel test tak terpakai yang
ternyata memang seharusnya diassert).

**Keputusan.** `clippy --workspace --all-targets -- -D warnings` jadi gate
yang memblokir sejak hari pertama. `cargo-deny` dipasang dengan
`continue-on-error: true` karena backlog advisory/lisensinya belum pernah
dibereskan — gate merah sejak hari pertama hanya melatih orang
mengabaikannya. Hapus penanda itu begitu `cargo deny check` hijau sekali.

---

## 4. Toolchain: `stable`, belum dipaku ke versi eksak

**Keputusan.** `rust-toolchain.toml` menjamin komponen `rustfmt` + `clippy`
tersedia, tetapi `channel` masih `"stable"`.

**Alasan.** Memaku versi eksak memang yang benar untuk reproduktifitas
(rilis stable baru bisa menambah lint dan membuat `-D warnings` mendadak
merah tanpa perubahan kode), **tetapi** mengganti channel mengubah
fingerprint build sehingga OCCT dikompilasi ulang dari nol (~10-15 menit),
dan versi yang hendak dipaku belum terpasang di mesin pengembang. Lakukan
pemakuan sebagai PR tersendiri saat rebuild penuh tidak mengganggu.

---

## 5. `panic = "unwind"` di profil rilis — bukan `"abort"`

**Keputusan.** Profil rilis memakai `lto = "thin"`, `codegen-units = 1`,
dan secara **eksplisit** `panic = "unwind"`.

**Alasan.** `ducad_kernel::lock_kernel()` dirancang untuk **pulih** dari
`KERNEL_LOCK` yang ter-*poison* akibat panic pada operasi kernel
sebelumnya, supaya satu operasi gagal tidak mengunci permanen seluruh
aplikasi. Mekanisme itu hanya berfungsi bila panic dapat di-*unwind*.
`"abort"` akan mematikan aplikasi CAD berisi pekerjaan pengguna yang belum
tersimpan — harga yang jauh lebih mahal daripada selisih ukuran binernya.

`lto` sengaja `"thin"`, bukan `"fat"`: graf dependensi menarik OCCT
(ratusan objek C++) bersama wgpu/egui, sementara kode C++ itu tidak ikut
ter-LTO dari sisi Rust sama sekali — jadi biaya link fat LTO tidak
terbayar.

---

## 6. Undo lintas domain: `Transaction`, bukan penggabungan tumpukan

**Masalah.** `ducad-app` memegang satu `UndoStack<Sketch>` **per bidang
sketsa** ditambah satu `UndoStack<ModelDoc>` terpisah. Satu aksi pengguna
yang menyentuh dua domain ("gambar profil lalu extrude") jadi dua langkah
undo yang tak berhubungan, dan domain yang lebih baru (assembly, datum
plane, drawing sheet) tidak punya undo sama sekali.

**Keputusan.** `ducad_core::undo` kini menyediakan:
- **`Transaction<T>`** — satu langkah undo berisi banyak `Command`.
  `apply` maju, `revert` mundur (urutan terbalik; command belakangan boleh
  bergantung pada efek command sebelumnya). `Transaction` sendiri
  mengimplementasikan `Command`, jadi bersarang tanpa perlakuan khusus.
- **Coalescing** lewat `Command::coalesce_key() -> Option<(&'static str, u64)>`
  (tag jenis + id sasaran). Default `None`, sehingga **tidak satu pun**
  implementasi yang sudah ada berubah perilaku.
- **Batas kedalaman** (default 200). Tumpukan lama tumbuh tanpa batas,
  padahal command modeling menyimpan snapshot B-rep — kebocoran memori
  yang nyata pada sesi panjang.

**Alternatif yang ditolak.**
- *`dyn Any` + downcast untuk menggabungkan dua command sejenis.* Merusak
  object-safety `Command` dan memaksa setiap implementasi menambah
  `as_any_mut`. Kunci `(tag, id)` mencapai hasil yang sama tanpa itu.
- *Menimpa isi command saat coalescing.* Penggabungan dilakukan dengan
  **menambahkan** command ke transaksi puncak, sehingga `revert` tetap
  memutar balik seluruh rantai sampai keadaan sebelum drag — dan tidak ada
  command yang perlu tahu cara menggabungkan dirinya dengan command lain.

**Belum selesai.** Transaksi lintas domain sungguhan (sketsa + fitur +
body dalam satu langkah) menunggu **P0.1**, karena `Sketch` dan `ModelDoc`
masih dua target `Command<T>` yang berbeda. Model dokumen tunggal adalah
prasyaratnya.
