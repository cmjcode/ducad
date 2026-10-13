---
tags: [ducad, standard]
---
# Paket Aturan DUCAD

Paket aturan = kumpulan check bersitasi yang dipasang lewat
`set_checks {"rule_packs": [...]}`. Daftar terkini + variabelnya selalu dari
`get_schema {"rule_packs": true}`; tabel ini hanya peta.

| Paket | Kapan dipakai | Variabel yang sering diubah |
|---|---|---|
| `dfm-cnc` | part di-mill 3 sumbu | `min_internal_radius` (radius pahat terkecil), `axis`, `min_thread_ratio` |
| `dfm-sheet-metal` | potong laser + tekuk | `thickness` (pakai `"$t"`), `min_bend_ratio` |
| `dfm-fdm` | cetak FDM | `up` (arah cetak), `max_overhang_area` |
| `dfm-injection` | cetak injeksi | `pull` (arah buka cetakan), `min_draft_deg` |
| `iso-2768` | kelas toleransi umum vs proses | `class`, `process` |
| `fea-safety` | ada beban di brief | `min_safety_factor`, `study` |

Cara membaca hasil:
- Sitasi berawalan `Guideline:` = pedoman praktik bengkel, bukan isi standar.
  Sitasi tanpa awalan itu merujuk standar (mis. ISO 2768-1 Tabel 1).
- `severity: warning` dilaporkan tetapi tidak menghalangi build; jelaskan ke
  user mengapa diterima atau perbaiki desainnya.
- Angka batas milik user/firma: bila user menyebut nilai lain (mis. pahat
  terkecil Ø3 → `min_internal_radius: 1.5`), pakai `vars` dan catat di
  `Preferences/Defaults.md`.
- Laporan kepatuhan adalah check otomatis; engineer yang berwenang tetap
  harus mereview dan menandatangani.
