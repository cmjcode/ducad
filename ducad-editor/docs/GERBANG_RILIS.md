# Gerbang rilis workflow designer

Step 5 `docs/PLAN_DESIGNER_UX.md`. Rilis hanya boleh dibuat bila ketiga
bagian di bawah lulus. Klaim di README dan panduan harus sesuai
[`STATUS_KEMAMPUAN.md`](STATUS_KEMAMPUAN.md) dan
[`FORMAT_MATRIX.md`](FORMAT_MATRIX.md); kemampuan yang tidak lolos gerbang
tidak boleh disebut `stabil`.

## 1. Tes otomatis inti (memblokir)

```sh
scripts/release-gate.sh        # dari root repo
```

Skrip menjalankan tes yang memetakan workflow sasaran (bracket contoh di
`ducad_engine::samples`, juga dipakai tutorial) per metrik kualitas, mencetak
tabel, dan keluar non-nol bila ada yang gagal atau ada kelompok tes yang tidak
menemukan tes sama sekali (nama tes berubah). CI menjalankannya di lane
**designer** (`.github/workflows/ci.yml`), yang memblokir merge; tabelnya
tampil di ringkasan job.

| Metrik | Yang diuji |
|---|---|
| Workflow proyek contoh (oplog) | `sample_project.rs` (buat, ubah dimensi awal, pulih dari op gagal, undo, simpan/buka, ekspor, drawing), `samples` |
| Regresi simpan/buka | `session_file.rs`, `format_validation.rs` (`native_*`, `corrupt_*`), `ducad-io/tests/native_format.rs` |
| Kegagalan ekspor | `format_validation.rs` (`step_*`, `stl_*`, `drawing_*`), tes DXF `ducad-io` |
| Crash / error fatal | `kernel_budget.rs` (kernel berbatas waktu), `hostile.rs` (input acak tidak panic) |
| Tutorial dan saran GUI | `onboarding_ui` (bab proyek contoh, sisihan dokumen kerja, pemulihan setelah crash), `next_action`, ringkasan ekspor, `onboarding_render` (kartu sambutan) |
| Teks UI | `ui_glyph_coverage` (tidak ada glyph "kotak"), paritas terjemahan `ducad-i18n` |

Lane CI lain (`full`: seluruh workspace) tetap harus hijau. Sejak Step 5
seluruh `cargo test --workspace` lulus (kegagalan lama vault ISO dan
layering `ducad-update`/`ducad-cloud` sudah diperbaiki).

## 2. Checklist GUI manual (memblokir rilis, bukan merge)

Di platform utama (desktop macOS/Windows, mouse):

- [`CEKLIS_UJI_GUI.md`](CEKLIS_UJI_GUI.md) **Bagian B** (regresi inti).
- **Bagian P.5** (tutorial, sisihan dokumen, saran, label ekspor).
- **Bagian P** T0–T6 dengan minimal satu penguji baru per rilis besar.

## 3. Metrik kualitas rilis

Isi satu baris per rilis. Angka otomatis dari keluaran `release-gate.sh`;
angka manual dari ringkasan sesi `CEKLIS_UJI_GUI.md` P.4.

| Rilis | Tanggal | Tugas mandiri (P.4) | Crash / error fatal (tes + sesi) | Regresi simpan/buka | Kegagalan ekspor | Catatan |
|---|---|---|---|---|---|---|
| (baseline Step 5) | 2026-10-21 | belum diukur (butuh penguji manusia) | 0 gagal / 4 tes | 0 gagal / 19 tes | 0 gagal / 25 tes | Gate otomatis 100 tes lulus; `cargo test --workspace` 1.901 lulus, 0 gagal; logika P.5 teruji otomatis, konfirmasi di jendela asli belum |

Target (PLAN §6): ≥ 4 dari 5 penguji menyelesaikan proyek contoh; 0
kehilangan pekerjaan; 0 regresi pada fixture; semua format `stabil` lulus
validasi otomatis.

## 4. Urutan rilis

1. `scripts/release-gate.sh` lulus di mesin rilis (macOS: juga
   `cargo test --workspace`, karena CI tidak punya lane workspace macOS).
2. Checklist §2 lulus; baris metrik §3 diisi.
3. `scripts/release-tag.sh` (tag `v*` memicu `release.yml`).
