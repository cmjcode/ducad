# ADR 0008 — Paket desain: brief, kepatuhan bersitasi, dan gerbang rilis

Status: diterima (2026-10-13). Rencana: `.claude/plans/ducad-arsitektur-blueprints.plan.md`.

## Konteks

Alur kerja yang ditiru adalah alur "agent menghasilkan dan merevisi satu set
dokumen yang konsisten, terverifikasi terhadap aturan, dengan manusia sebagai
gerbang" (pembanding: Blueprints AI di arsitektur), diterapkan pada desain
teknik mesin. Sebelum ADR ini DUCAD punya potongannya (oplog, checks,
`DrawingSpec`, `propose_ops`, vault memori), tetapi tidak punya: brief
tersimpan, keterlacakan persyaratan, sitasi pada check, aturan DFM, set
dokumen multi-lembar, profil firma, redline, dan status persetujuan.

## Keputusan

1. **Brief, status rilis, dan susunan paket adalah data dokumen, bukan op.**
   `DesignDoc.{brief, release, package}` mengikuti pola `drawings` (ADR 0007):
   mengubahnya tidak mengubah geometri, jadi tidak masuk oplog dan tidak
   membatalkan replay.
2. **Keterlacakan lewat satu field.** `CheckItem.requirement` menaut ke
   `Requirement.id`. Matriksnya dihitung, tidak disimpan
   (`brief::traceability`), sehingga tidak bisa basi.
3. **Kepatuhan = check deterministik + sitasi.** `CheckItem` mendapat
   `citation`, `severity` (`warning` tidak menghalangi build), dan `rule`.
   Paket aturan adalah berkas data (`rules/*.json`) yang dikembangkan menjadi
   check konkret; angka batas adalah variabel paket, bukan konstanta di kode.
   Setiap aturan wajib menyatakan `basis`: `standard` hanya bila yang ditulis
   memang isi standar (mis. tabel ISO 2768-1), selain itu `guideline` dengan
   sitasi berawalan `Guideline:`. Tes menolak aturan tanpa sitasi.
4. **Tidak ada klaim sertifikasi.** Setiap laporan dan sampul paket memuat
   `RULE_DISCLAIMER`; instruksi server MCP melarang agent menyebut desain
   tersertifikasi.
5. **Satu PDF multi-halaman.** `generate_pdf_document` ditambahkan di samping
   `generate_pdf_bytes` (yang tidak diubah, agar hash emas P21 tetap). Halaman
   non-gambar (sampul, BOM, kepatuhan, pola bentangan) adalah lembar kosong +
   grup display-list (`drawing::pages`), jadi bingkai dan kepala gambarnya
   sama persis dengan lembar gambar.
6. **Persetujuan terikat isi desain.** `ReleaseInfo.approval.signature` =
   hash (params, oplog, konfigurasi, checks) — ditambah sidik jari geometri
   bila part punya `base_bodies`. Desain berubah → tanda tangan berbeda →
   status efektif kembali draf, tanpa perlu mengait setiap jalur mutasi.
   Pengajuan berikutnya menaikkan revisi (A → B).
7. **Persetujuan hanya oleh manusia.** `approve_package` tersedia di server
   MCP headless (CI/skrip pengguna) tetapi ada di `attach::UNSUPPORTED`; di
   GUI hanya tombol panel Review & Rilis; di terminal `ducad-cli release`.
8. **Redline = anotasi lembar.** `Annotation::Redline` disimpan di
   `DrawingSpec.layout.annotations`, digambar merah oleh PDF/SVG/kanvas dari
   geometri yang sama, dan tidak digambar lagi setelah `resolved`. Redline
   merujuk titik di kertas, bukan topologi, jadi tidak menunggu penamaan
   topologi (P0.4).
9. **Biaya bersifat indikatif dan bersumber dari pengguna.** Tidak ada harga
   bawaan: tanpa `firm.json`, laporan biaya hanya memuat massa dan volume stok.

## Konsekuensi

- `tools/list` MCP melewati batas 40 KB bila skema check ditanam; skema itu
  kini diambil lewat `get_schema {"checks": true}` (pola yang sama dengan op).
- GUI kini membawa bagian non-op dokumen ke `agent_meta` saat berkas dibuka
  (`adopt_loaded_design`); sebelumnya checks berkas bisa tertimpa keadaan
  dokumen sebelumnya begitu tool agent dipanggil.
- Lembar yang tidak terdampak revisi identik byte demi byte hanya bila kepala
  gambarnya sama; revisi baru mengubah huruf revisi di semua lembar. `diff`
  melaporkan `impact` (lembar/check/persyaratan terdampak) untuk reviewer.

## Yang sengaja tidak dikerjakan

- Impor PDF hasil pindai (raster) dan point cloud.
- Estimasi biaya dari fitur (jumlah setup, panjang potong); modelnya hanya
  volume × laju.
- Deteksi sudut dalam tajam di luar arah sumbu pahat tunggal.
- Paket aturan per yurisdiksi/kode industri (ASME BPVC, Eurocode): kerangka
  datanya siap, isinya harus disusun dan ditinjau engineer.
