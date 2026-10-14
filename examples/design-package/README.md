# Contoh paket desain

Alur "brief → set dokumen patuh standar → review → revisi" untuk dua part.

| Berkas | Isi |
|---|---|
| `sensor_bracket.ops.json` | Bracket CNC: brief + 5 persyaratan, check tertaut, paket aturan `dfm-cnc`, studi statik |
| `tray.ops.json` | Baki sheet metal: paket aturan `dfm-sheet-metal`, pola bentangan |
| `plate_review.ops.json` | Plat sederhana tanpa studi, untuk uji GUI (ceklis A16 di `ducad-editor/docs/CEKLIS_UJI_GUI.md`) |
| `firm.json` | Profil firma contoh: kepala gambar, harga material, tarif proses, stok pelat |
| `set.json` | Kedua part sebagai satu set |

```sh
# Satu part: PDF multi-halaman + traceability.md + compliance.md + cost.md
ducad-cli build sensor_bracket.ops.json --out out --package --date 2026-10-13

# Seluruh set
ducad-cli build set.json --out out-set

# Gerbang rilis
ducad-cli run sensor_bracket.ops.json --out bracket.ducad
ducad-cli build bracket.ducad --out rel --package --release   # kode 4: masih draf
ducad-cli release bracket.ducad --submit "Initial release" --by ana
ducad-cli release bracket.ducad --approve --by budi
ducad-cli build bracket.ducad --out rel --package --release   # kode 0

# Revisi: persetujuan gugur begitu desain berubah
ducad-cli replay bracket.ducad --param t=5 --out bracket.ducad
ducad-cli diff bracket.ducad bracket.ducad                     # lihat `impact` antar dua berkas revisi
ducad-cli release bracket.ducad --submit "Base thinned to 5 mm" --by ana   # revisi B
```

Mencoba `--param t=8` memperlihatkan gunanya keterlacakan: massa menjadi
92,8 g, persyaratan R3 (di bawah 80 g) gagal, dan revisi itu tidak bisa
disetujui.

Harga dan tarif di `firm.json` adalah angka contoh. Laporan biaya bersifat
indikatif, dan semua check bersifat otomatis: paket tetap harus direview dan
ditandatangani engineer yang berwenang.
