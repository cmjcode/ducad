# Validasi simulasi DUCAD

Hasil simulasi DUCAD adalah **estimasi rekayasa**. Dokumen ini mencatat
seberapa dekat solver dengan solusi analitik, supaya angka di laporan bisa
ditimbang dengan benar. Semua kasus di bawah dijalankan otomatis oleh
`cargo test -p ducad-sim`; tes gagal bila hasil keluar dari toleransi.

## Statik linier, mesh hex voxel (P17)

Elemen Hex8 dengan mode inkompatibel, sel hampir kubus. Label di UI dan
laporan: "Estimasi (hex)".

| Kasus | Mesh | Terukur | Analitik | Selisih | Toleransi gerbang |
|---|---|---|---|---|---|
| Batang tarik 10×10×100, 1000 N — tegangan | sel 2,5 mm | 10,0000 MPa | 10 MPa | 0,00 % | ±2 % |
| Batang tarik — pertambahan panjang | sama | 0,004744 mm | 0,004762 mm | −0,37 % | ±3 % |
| Kantilever 10×10×100, 100 N — defleksi ujung | sel 2,5 mm | 0,190113 mm | 0,190476 mm | −0,19 % | ±10 % |
| Kantilever — von Mises maksimum | sama | 61,118 MPa | 60 MPa | +1,86 % | ±10 % |
| Pelat tumpu sederhana 80×80×2, tekanan merata — defleksi tengah | sel 2 mm, satu lapis | 0,010997 mm | 0,010809 mm | +1,74 % | ±10 % |
| Pelat berlubang 100×50×2, Ø10, tarik — faktor konsentrasi | sel 1 mm | 2,834 | 3,149 (Howland, lebar hingga) | −10,0 % | 2,5–3,3 |

## Mesh tetra, frekuensi, buckling, termal (P18)

| Kasus | Mesh | Terukur | Acuan | Selisih |
|---|---|---|---|---|
| Kantilever — defleksi ujung | Tet10 h = 5 / 3,5 / 2,5 | 0,189822 / 0,190201 / 0,190385 mm | 0,190476 mm | −0,34 / −0,14 / −0,05 % |
| Kantilever — tegangan serat atas di x = 10 | sama | 54,409 / 54,158 / 54,117 MPa | 54 MPa | +0,76 / +0,29 / +0,22 % |
| Pelat berlubang — faktor konsentrasi | Tet10 h = 3 | 3,2176 | 3,1488 (Howland, d/W = 0,2) | +2,19 % |
| Frekuensi kantilever 150×10×5, mode 1–3 | Tet10 h = 5 | 186,72 / 371,42 / 1164,78 Hz | 185,67 / 371,34 / 1163,58 Hz | +0,56 / +0,02 / +0,10 % |
| Frekuensi yang sama | hex 2,5 | 186,27 / 371,23 / 1162,71 Hz | sama | +0,32 / −0,03 / −0,07 % |
| Tekuk Euler kolom 100×10×5 (sendi-sendi) | Tet10 h = 5 | 21112,8 N | 21589,8 N | −2,21 % |
| Tekuk yang sama | hex 2,5 | 21194,6 N | sama | −1,83 % |
| Batang termal — profil suhu | Tet10 h = 5 | galat maks 4,9e-11 K | linier | — |
| Batang termal — tegangan muai bebas | Tet10 h = 5 | 1,76e-8 MPa | 0 | — |

Catatan:
- Terhadap pelat tak hingga (Kt = 3,0) hasil pelat berlubang +7,3 %, di luar
  ±5 %; acuan yang dipakai adalah faktor lebar-hingga.
- Von Mises maksimum global kantilever naik dengan penghalusan (+0,61 / +3,95 /
  +5,31 % terhadap 60 MPa) karena singularitas akar jepit; gate memakai
  penampang x = 10 mm.
- Kualitas mesh: 7 fixture sintetis, nol tet terbalik, radius-ratio minimum
  0,200–0,204, galat volume ≤ 0,0104 %.
- Mesh hex memberi 4,5 MPa tegangan palsu pada batang muai bebas dengan
  gradien 20→120 °C; suhu seragam eksak.
- Lewat engine (`tests/study_kinds.rs`): balok Al 6061 100×10×5 — frekuensi
  pertama, faktor tekuk jepit-bebas, dan σ = EαΔT cocok dengan rumus dalam 5 %.

## Batas yang perlu diketahui

- **Puncak tegangan bergantung mesh.** Sudut bertangga voxel membuat nilai
  maksimum di tepi lubang dan di akar jepit naik saat mesh diperhalus: faktor
  konsentrasi pelat berlubang terukur 2,72 (sel 2 mm), 2,83 (1 mm), 2,98
  (0,77 mm), dan 3,30 (0,5 mm). Tegangan penampang jauh dari singularitas
  tetap dalam ±0,5 %. Gunakan nilai maksimum sebagai indikasi, bukan angka
  sertifikasi.
- **Dinding tipis.** Dinding yang hanya 1–2 sel tebalnya masih memberi
  defleksi yang wajar, tetapi tegangannya kasar; perkecil `mesh.cell_mm`.
- **Hanya linier statik, material isotropik.** Tanpa plastisitas, kontak,
  lendutan besar, fatigue, maupun beban dinamik.
- **Tumpuan `roller`/`symmetry` miring** terimplementasi tetapi baru diuji
  untuk face sejajar sumbu.
- **Torsi** diterapkan sebagai kopel murni terhadap centroid face.

## Kinerja (rilis, satu mesin pengembang, tidak terisolasi)

| Kasus | Elemen | DOF | Iterasi PCG | Waktu `run_static` |
|---|---|---|---|---|
| Kantilever | 49.419 | 166 rb | 142 | 1,01 s |
| Pelat berlubang tebal 10 mm | 50.200 | 170 rb | 127 | 0,99 s |
| Pelat berlubang tipis 1 mm | 48.520 | 221 rb | 340 | 2,82 s |

Diukur dengan `cargo test -p ducad-sim --release --test benchmarks -- --ignored --nocapture`
sementara build lain berjalan di mesin yang sama; perlakukan sebagai perkiraan.
Pengukuran iPad belum dilakukan.

### Tet10 (P18)

| Langkah (bracket L, 94.122 Tet10, 395 rb DOF) | Waktu | Target |
|---|---|---|
| Meshing | 0,71 s | — |
| Statik (296 iterasi CG) | 6,04 s (6,75 s dengan meshing) | < 10 s — tercapai |
| 10 mode frekuensi (175 iterasi LOBPCG) | 247 s | < 30 s — **meleset** |

Bracket yang sama pada 14 rb elemen: 18 s untuk 10 mode, tiga frekuensi
pertama dalam 0,4 % dari mesh 94 rb.
