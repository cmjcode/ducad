# CI/CD untuk part DUCAD

`ducad-cli build` mengubah satu part (`.ops.json` atau `.ducad`) menjadi
artefak manufaktur dan laporan, cocok untuk dijalankan di setiap push/PR.

## Struktur repo yang dianjurkan

```
parts/
  bracket.ops.json      # sumber kebenaran: params + ops + checks
  plate.ops.json
.gitattributes          # *.ducad diff=ducad
.github/workflows/ducad-build.yml   # salinan docs/ci/ducad-build.yml
```

Simpan part sebagai `*.ops.json` (teks, ramah diff). Bila memakai `.ducad`,
pasang textconv agar `git diff` menampilkan oplog alih-alih JSON geometri:

```sh
echo '*.ducad diff=ducad' >> .gitattributes
git config diff.ducad.textconv 'ducad-cli oplog'
```

## Perintah

```
ducad-cli build <PART.ducad | OPS.json> --out DIR [--formats LIST] [--paper a4|a3]
               [--title T] [--part-number PN] [--author A] [--revision R]
               [--date YYYY-MM-DD] [--no-checks]
               [--drawing NAME] [--section LABEL:INDUK:SUMBU:OFFSET[:flip]]...
               [--shaded iso,iso_back] [--scale auto|1:2|0.5]
```

- **Lembar gambar (P21).** Bila part memuat lembar tersimpan
  (`design.drawings` di `.ducad`, atau `"drawings": [...]` di OpFile), lembar
  pertama — atau `--drawing NAME` — dirender apa adanya: tampak, potongan,
  skala per tampak, render berbayang, catatan, dimensi asosiatif. Tanpa lembar
  tersimpan: 4 tampak + potongan A-A + dimensi otomatis seperti sebelumnya.
- `--section A:top:x:0` (boleh diulang) mengganti potongan; `--shaded` mengganti
  render berbayang; `--scale` mengganti skala lembar; `--paper`, `--title`,
  `--part-number`, `--author`, `--revision` menimpa kepala gambar. Tanggal
  selalu dari `--date`/`SOURCE_DATE_EPOCH`.
- Peringatan lembar (`HLR_EXACT_FALLBACK`, `DRAWING_SECTION_EMPTY`,
  `DRAWING_DXF_NO_RASTER`) masuk `report.json.warnings`. Potongan eksplisit
  yang tidak memotong body menghentikan build (`drawing_section_empty`).

- `LIST` default `step,stl,pdf,png,bom`; pilihan: `step, stl, obj, glb, pdf, svg, dxf, png, bom`.
- Keluaran di `DIR`: `<stem>.step|stl|obj|glb`, `<stem>-drawing.pdf|svg|dxf`,
  `<stem>-iso.png`, `<stem>-bom.csv`, `report.json`, `report.md`.
- Kode keluar: `0` sukses · `1` op/replay gagal (termasuk `.ducad` yang tidak
  dapat direproduksi dari oplog-nya) · `2` salah pakai · `3` ada check gagal
  (laporan tetap ditulis, artefak **tidak**) · `4` `--release` pada paket
  yang belum disetujui.
- Deterministik: masukan dan tanggal sama → byte keluaran sama. Tanggal
  diambil dari `--date`, lalu env `SOURCE_DATE_EPOCH`, lalu hari ini. Set
  `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)` agar artefak hanya berubah
  bila part berubah.
- Kertas A2 belum didukung.

## Paket desain

Format `package` (atau `--package`) menulis SATU PDF multi-halaman dan tiga
laporan di samping artefak biasa:

```
ducad-cli build PART --out DIR --package [--firm firm.json] [--rule-pack NAMA]...
               [--project-note PATH] [--release]
```

- `<stem>-package.pdf`: sampul (brief, indeks lembar, tabel revisi, ringkasan
  kepatuhan), gambar part per body, gambar assembly berbalon BOM, pola
  bentangan tiap body sheet metal, lembar BOM, lembar kepatuhan. Susunan
  diatur `"package": {"sheets": [...]}` di OpFile/`.ducad`; tanpa itu otomatis.
- `traceability.md` (persyaratan → check), `compliance.md` (tiap check +
  sitasi), `cost.md` (biaya indikatif dari profil firma; bukan penawaran).
- OpFile boleh memuat `"brief"`, `"rule_packs"`, dan `"requirement"`/
  `"citation"`/`"severity"` per check. Check `severity: "warning"` yang gagal
  dilaporkan tetapi tidak membuat kode keluar 3.
- **Profil firma**: `--firm`, lalu env `DUCAD_FIRM`, lalu `firm.json` di
  samping part, lalu `~/DUCAD-Memory/Standards/firm.json`. Mengisi kepala
  gambar, teks toleransi umum, catatan standar, harga material, tarif proses,
  dan paket aturan wajib (`rule_packs`).
- **Gerbang rilis**: `--release` keluar dengan kode `4` (status
  `not_released`, tanpa artefak) kecuali paket berstatus `approved` untuk
  desain yang sedang dibangun dan tidak ada check penghalang atau redline
  terbuka. Persetujuan dilakukan manusia:

  ```sh
  ducad-cli release PART.ducad --submit "Initial release" --by ana
  ducad-cli release PART.ducad --approve --by budi
  ducad-cli release PART.ducad            # status + ceklis review
  ```

  Mengubah desain setelah disetujui membatalkan persetujuan; `--submit`
  berikutnya membuka revisi B.
- **Set part**: `ducad-cli build SET.json --out DIR` dengan
  `{"title": "…", "parts": ["a.ops.json", "b.ducad"]}` membangun tiap part ke
  `DIR/<stem>/` dan menulis ringkasan `DIR/report.md`.

Contoh siap jalan: `examples/design-package/` (bracket CNC + baki sheet metal
+ `firm.json`).

## Menempel `report.md` ke PR

`report.md` dibatasi ±60 baris. Tambahkan langkah berikut setelah build:

```yaml
      - if: github.event_name == 'pull_request'
        env: { GH_TOKEN: "${{ github.token }}" }
        run: |
          for r in dist/*/report.md; do
            gh pr comment "${{ github.event.pull_request.number }}" --body-file "$r"
          done
```

(Butuh `permissions: pull-requests: write` pada job.)

## Binary

Rilis `v*` repo DUCAD (workflow `.github/workflows/release.yml`) memuat:

- `ducad-tools-linux-x86_64.tar.gz` dan `ducad-tools-windows-x86_64.zip`:
  `ducad-cli` + `ducad-mcp` saja, untuk CI (templat di atas memakai yang Linux);
- `ducad-<versi>-linux-x86_64.tar.gz` dan `ducad-<versi>-windows-x86_64.zip`:
  aplikasi GUI `ducad` beserta kedua binary headless;
- `*.sha256` untuk setiap berkas.

Paket macOS tidak dibangun CI: bundel `.app` ditandatangani dan dinotarisasi
di mesin lokal (`make bundle-macos`, `./notarize.sh`), lalu diunggah manual ke
rilis yang sama dengan `gh release upload v<versi> <berkas> --clobber`.
