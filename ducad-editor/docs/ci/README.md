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
  (laporan tetap ditulis, artefak **tidak**).
- Deterministik: masukan dan tanggal sama → byte keluaran sama. Tanggal
  diambil dari `--date`, lalu env `SOURCE_DATE_EPOCH`, lalu hari ini. Set
  `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)` agar artefak hanya berubah
  bila part berubah.
- Kertas A2 belum didukung.

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

Rilis `v*` repo DUCAD memuat `ducad-tools-macos-arm64.tar.gz` (berisi
`ducad-cli` dan `ducad-mcp`). Build Linux x86_64 belum dirilis: workflow CI
utama sudah membangun OCCT di Ubuntu, tetapi paket rilisnya belum diuji.
