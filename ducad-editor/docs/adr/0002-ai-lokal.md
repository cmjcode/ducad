# ADR 0002 — AI lokal/offline di DUCAD (P11.0)

**Status**: Diterima · **Tanggal**: 2026-09-20 · **Cakupan**: P11.0–P11.4

Spike ini menjawab empat pertanyaan P11.0 dengan pengukuran di mesin
pengembangan (Apple M2, 24 GB, macOS 27.0 build 26A428, Swift 6.4).
Angka di bawah berasal dari eksekusi nyata, bukan perkiraan. Apa yang
**tidak** bisa diukur di sini disebut apa adanya.

---

## 1. Apakah Rust/eframe bisa memanggil Foundation Models Apple?

**Bisa di macOS — terbukti jalan, bukan teori.**

`crates/ducad-assist/swift/DucadFM.swift` dikompilasi `swiftc` menjadi
pustaka **statis** ber-ABI C (`@_cdecl`), lalu ditautkan ke binary Rust
lewat `build.rs`. `ducad-cli assist --backend apple` menghasilkan usulan
yang valid dari model sistem di perangkat.

Detail yang perlu diketahui saat memindahkannya ke proyek Xcode:

- **Versi OS minimum**: Foundation Models ada sejak macOS 26 / iOS 26.
  Kode memakai `#available` dan `canImport(FoundationModels)`, jadi tetap
  terkompilasi untuk deployment target lebih lama.
- **Perangkat tanpa dukungan**: `SystemLanguageModel.default.availability`
  dibaca saat jalan. `ducad_fm_available()` mengembalikan `false` (OS lama,
  Apple Intelligence mati, perangkat tidak didukung) dan backend **tidak
  ditawarkan** di pengaturan; `ducad_fm_status()` memberi alasannya.
- **Jebakan penautan**: deployment target default rustc adalah macOS 11,
  sedangkan `libswift_Concurrency.dylib` baru ada di OS sejak macOS 12.
  Linker lalu memakai install name `@rpath/…` dan binary gagal dimuat
  ("no LC_RPATH's found"). Perbaikannya ada di `crates/ducad-cli/build.rs`:
  tambahkan `-Wl,-rpath,/usr/lib/swift` untuk binary. Untuk bundel yang
  dikirim ke macOS lama, framework-nya harus **weak-linked**
  (`-weak_framework FoundationModels`) di proyek Xcode — belum dikerjakan.
- **Async**: `respond(to:)` bersifat async; jembatan memblokirnya dengan
  semaphore, jadi `complete()` **wajib** dipanggil dari thread latar. GUI
  sudah melakukannya (`assist_ui.rs` menjalankan `assist` di thread sendiri).
- **iPadOS belum diuji**: tidak ada perangkat iPad dalam sesi ini. Kode
  Swift dan jalur C-nya sama, tetapi penautan di proyek Xcode iOS dan
  perilaku di perangkat **belum dibuktikan**.

## 2. Tingkat lulus pada dua tugas P6

Prompt P11.2 apa adanya, checks tugas eval sebagai penilai, `--accept` lalu
`ducad-cli check`. `param_edit` juga memeriksa `oplog_len = 4`.

| Backend | Ukuran | `param_edit` | `plate_4holes` | Waktu/tugas | RAM puncak |
|---|---|---|---|---|---|
| Apple Foundation Models | model sistem | **5/5** | 0/5 | 3,4–11 s | — (proses sistem) |
| Qwen2.5-0.5B-Instruct Q4_K_M | 491 MB | **3/3** | 0/3 | ±24 s | 1,79 GB |
| Qwen2.5-1.5B-Instruct Q4_K_M | 1,12 GB | **3/3** | 0/3 | ±19 s | 3,75 GB |

`param_edit` ≥ 50% pada ketiganya → **lingkup P11 tidak dipersempit**.

`plate_4holes` (membuat part baru dari nol) gagal di semua backend, dengan
pola kegagalan yang konsisten: model menyusun aksi yang tidak ada
(`{"sketch": …}` sebagai aksi), memberi larik ke `set_params`, atau memakai
id yang sama untuk beberapa op. Ini yang sudah diperkirakan plan: membuat
part rumit adalah pekerjaan agent eksternal, dan batas itu dinyatakan di UI
(`assist-capability-note`).

Dua pelonggaran parser lahir dari pengamatan ini (keduanya ada tesnya):

1. Balasan berisi **dua** objek JSON berturut-turut → objek pertama dipakai.
   Aturan P11.2 ("`{` pertama s.d. `}` terakhir") sendirian menolak balasan
   yang isinya sebenarnya benar.
2. Balasan berupa **satu aksi telanjang** tanpa pembungkus
   `{rationale, actions}` → dibungkus otomatis. Tanpa ini Qwen2.5-1.5B lulus
   0/3 `param_edit` padahal isi usulannya benar; dengan ini 3/3.

Pesan error bentuk-salah juga diperbaiki agar menyebut **aksi ke berapa**
yang salah — pesan serde mentah ("invalid type: sequence") tidak membantu
model kecil.

## 3. Lisensi model (diperiksa di halaman modelnya, bukan diasumsikan)

| Model | Lisensi | Boleh jadi default? |
|---|---|---|
| Qwen2.5-0.5B-Instruct-GGUF | `apache-2.0` | ya |
| Qwen2.5-1.5B-Instruct-GGUF | `apache-2.0` | ya |
| Qwen2.5-3B-Instruct-GGUF | `qwen-research` (non-komersial) | **tidak** |

Persis jebakan yang diingatkan plan: dalam satu keluarga model, ukuran yang
berbeda bisa berlisensi berbeda. **Model default = Qwen2.5-1.5B-Instruct
Q4_K_M** (Apache-2.0): pada beban ini sedikit lebih cepat daripada 0.5B
karena lebih jarang butuh iterasi ulang, dengan tingkat lulus sama. 0.5B
tetap menjadi pilihan untuk perangkat ber-RAM kecil.

## 4. Ukuran, RAM, dan kecepatan

- Unduhan: 491 MB (0.5B) / 1,12 GB (1.5B) + 6,7 MB `tokenizer.json`.
  Disimpan di `~/.ducad/models/`, **tidak dibundel**; crate `ducad-assist`
  tidak pernah menyentuh jaringan — unduhan hanya lewat dialog persetujuan
  yang menampilkan nama, ukuran, dan lisensi model (dialog itu **belum
  dibuat**; saat ini berkas disiapkan manual).
- RAM puncak proses: 1,79 GB (0.5B) dan 3,75 GB (1.5B).
- Kecepatan di M2 (CPU, candle, tanpa Metal): prompt ±1.400 token menghabiskan
  sebagian besar waktu (prefill ±20 s), keluaran ±2,8 token/s pada 0.5B.
  Backend Apple jauh lebih cepat (3,4 s untuk `param_edit`) karena memakai
  akselerator sistem. **Kesimpulan praktis**: backend Apple adalah jalur
  utama di perangkat Apple; GGUF adalah cadangan untuk perangkat/OS yang
  tidak punya Foundation Models.
- Belum diukur: token/detik di iPad, dan pengaruh fitur `metal` pada candle
  (tidak diaktifkan di sesi ini).

## Keputusan

1. **Dua backend, keduanya mati secara default.** Fitur Cargo `apple-fm`
   dan `local-gguf` di `ducad-assist` (diteruskan oleh `ducad-cli` dan
   `ducad-app`), sehingga build biasa dan build iOS tidak terpengaruh.
2. **Apple Foundation Models jadi backend utama** di macOS/iPadOS ≥ 26;
   GGUF lokal sebagai cadangan. Model default GGUF: Qwen2.5-1.5B (Apache-2.0).
3. **Lingkup P11 penuh dipertahankan** (`param_edit` ≥ 50%), dengan batas
   kemampuan dinyatakan di UI: perubahan ukuran dan fitur sederhana, bukan
   part baru yang rumit.
4. **Keluaran selalu proposal.** `ducad-assist` tidak punya jalur kode yang
   meng-commit (dijaga tes yang memeriksa berkas sumbernya).
5. **`AiPrivacy::OfflineOnly` jadi default**: hanya backend `is_on_device()`
   yang bisa dipilih, tanpa telemetri isi desain, prompt, atau balasan.

## Yang belum dikerjakan (jujur)

- iPad: penautan pustaka Swift di proyek Xcode iOS dan pengukuran di
  perangkat.
- Weak-linking `FoundationModels` untuk bundel yang menargetkan macOS < 26.
- Dialog persetujuan unduh model (nama, ukuran, lisensi).
- P11.5 (memori MNEMONIC di iPad): perubahan lintas repo, butuh persetujuan
  pemilik repo — tidak disentuh.
