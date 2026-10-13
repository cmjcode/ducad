---
tags: [ducad, standard]
---
# Profil Firma (`firm.json`)

`Standards/firm.json` di vault ini adalah standar perusahaan yang dibaca
MESIN: `ducad-cli build --package`, tool `build_package`, dan resource MCP
`ducad://firm`. Berkas bawaan sengaja kosong; isi sesuai perusahaan user.
Profil per proyek (`firm.json` di samping part) mengalahkan yang di vault.

| Field | Guna |
|---|---|
| `company`, `drawn_by` | kepala gambar |
| `general_tolerance` | teks toleransi umum di kepala gambar, mis. `ISO 2768-m` |
| `paper` | kertas bawaan paket: `a4`, `a4-portrait`, `a3`, `a3-portrait` |
| `part_number_prefix` | awalan nomor gambar |
| `notes` | catatan yang selalu dicetak di lembar gambar |
| `materials` | `{name, aliases, price_per_kg}` untuk biaya indikatif |
| `processes` | `{process, setup_cost, rate_per_hour, rate_cm3_per_hour}` |
| `stock_thicknesses_mm` | tebal pelat yang distok (laporan menandai tebal di luar daftar) |
| `preferred_fasteners` | fastener yang didahulukan saat memilih `standard_part` |
| `rule_packs` | paket aturan yang selalu diterapkan, mis. `["dfm-cnc"]` |

Aturan untuk agent:
- Jangan mengarang harga atau tarif. Bila `materials`/`processes` kosong,
  laporkan bahwa biaya tidak bisa diestimasi dan tanyakan ke user.
- Perubahan profil hanya atas permintaan user; catat alasannya di
  `Preferences/Defaults.md`.
- Estimasi biaya selalu disebut **indikatif**, bukan penawaran harga.
