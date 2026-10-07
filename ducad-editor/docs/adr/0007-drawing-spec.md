# ADR 0007 — `DrawingSpec`: lembar gambar disimpan di `DesignDoc`, bukan sebagai op

**Status**: Diterima · **Tanggal**: 2026-10-07 · **Cakupan**: `ducad-io::drawing`,
`ducad-engine::{session, drawing_auto, tooling}`, `ducad-cli build`, editor lembar GUI

## Konteks

Sebelum P21 lembar gambar hanya hidup selama sesi GUI (`drawing_sheet_doc`)
dan tool `drawing` selalu membuat lembar bawaan. Untuk gambar produksi
(acuan: valve body A3 1:2 dengan potongan A-A bertingkat, B-B berskala
sendiri, dua render berbayang) dibutuhkan tiga hal sekaligus:

1. lembar yang sama bisa dihasilkan tanpa GUI, deterministik byte demi byte;
2. tata letak yang diatur pengguna tidak hilang saat berkas ditutup;
3. dimensi mengikuti geometri setelah `set_params`.

## Keputusan

1. **Satu tipe deklaratif `DrawingSpec`** (`ducad-io/src/drawing/spec.rs`):
   kertas, kepala gambar, catatan, potongan (`SectionSpec`), opsi per tampak
   (`ViewSet`), skala (`ScaleSpec`), render berbayang (`ShadedSpec`),
   kebijakan dimensi (`DimensionPolicy`), dan `layout` (posisi tampak, offset
   dimensi, teks, balon, BOM, anotasi GD&T). Tool `drawing`, `ducad-cli build`,
   dan editor GUI memakai tipe yang sama.
2. **Disimpan di `DesignDoc.drawings`, bukan di oplog.** Lembar gambar tidak
   mengubah geometri; menaruhnya di oplog akan (a) mengubah sidik jari part
   setiap kali sebuah dimensi digeser, (b) membuat undo geometri bercampur
   dengan undo tata letak, dan (c) memaksa `replay` menjalankan HLR. Ini
   konsisten dengan keputusan P19 bahwa anotasi bukan `Op`. `OpFile` mendapat
   field opsional `drawings` supaya `*.ops.json` tetap menjadi sumber tunggal.
3. **Jumlah tool tetap 27.** `drawing` menerima `name` + `save`; tanpa field
   lain ia merender lembar tersimpan.
4. **Dimensi menyimpan sumbernya, bukan posisinya.** `DimensionAnnotation`
   membawa `source: DimensionRef` (pusat lingkaran, tepi, sisi silinder, pola
   lubang) + `offset_mm`/`angle_deg`. Posisi kertas dihitung ulang dari situ.
   Rujukan fitur dicari lewat indeks edge topologi bila ada, lalu lewat
   petunjuk pusat/radius terdekat — sehingga tetap menempel walau radiusnya
   berubah. Yang disimpan di `layout` hanya dimensi yang digeser pengguna
   (`pinned`) dan dimensi manual.
5. **Satu display-list untuk semua ekspor** (`drawing::scene`). PDF, SVG, DXF,
   dan kanvas editor menerjemahkan primitif yang sama; itu yang menghapus bug
   "SVG tercermin sumbu-Y" dan menjamin skala per tampak konsisten.
6. **HLR eksak sebagai jalur utama** dengan fallback mesh + peringatan
   `HLR_EXACT_FALLBACK`. Potongan = satu boolean (prisma pemotong dari garis
   potong) + HLR atas sisa solid, di bawah satu guard `lock_kernel()`.
7. **Render berbayang dirender CPU** (`drawing::raster`), bukan wgpu, supaya
   gold test stabil lintas mesin; disematkan `/FlateDecode` di PDF dan PNG
   base64 di SVG.

## Konsekuensi

- `HlrDrawing.section_a`/`cutting_plane` dan `ProjectedViewKind::SectionAA`
  dihapus langsung (diganti `sections: Vec<SectionView>` dan `Section(char)`).
  Aman karena lembar belum pernah dipersistenkan sebelum ADR ini.
- Berkas `.ducad` yang disimpan GUI setelah lembar dibuka kini selalu membawa
  field `design` (dengan `base_bodies` bila belum ada oplog) agar spec ikut.
- Keluaran PDF/SVG lembar berubah total (hitam-putih ISO, teks dimensi tanpa
  " mm", catatan di atas kepala gambar); hash dasar di `gdt_pdf.rs` direkam
  ulang dan gold baru ada di `ducad-engine/tests/golden/drawing.hashes.json`.
- Dimensi otomatis yang DIHAPUS pengguna di editor tidak dipersistenkan: ia
  muncul lagi saat lembar dibangun ulang. Matikan lewat `dimensions`.

## Yang sengaja tidak dikerjakan

Tampak bantu miring, broken-out section, potongan miring (aligned), ekspor
gambar raster ke DXF, dan lembar rakitan multi-halaman.
