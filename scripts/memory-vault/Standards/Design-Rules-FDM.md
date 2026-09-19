---
tags: [ducad, standard]
---
# Aturan Desain Cetak FDM

Asumsi: nozzle 0,4 mm, tinggi layer 0,2 mm, PLA/PETG. Sesuaikan bila user
menyebut printer/material lain.

- Tebal dinding minimum 1,2 mm (3 perimeter); dinding struktural ≥ 2 mm.
- Fitur terkecil yang tercetak andal: 0,8 mm (lebar), 0,6 mm (tinggi timbul).
- Overhang tanpa support ≤ 45° dari vertikal; bridging ≤ 10 mm.
- Lubang vertikal tercetak lebih kecil ±0,2 mm: tambah Ø 0,2–0,3 mm atau
  bor ulang. Lubang horizontal: pertimbangkan bentuk tetes (teardrop).
- Celah rakit antar-part: 0,2 mm (pas), 0,3–0,4 mm (longgar/geser).
- Tepi bawah yang menempel bed: chamfer 0,5 mm alih-alih fillet (fillet di
  bawah menjadi overhang dan memperparah "elephant foot").
- Fillet tepi atas/samping R ≥ 1 mm mengurangi konsentrasi tegangan.
- Orientasikan beban sejajar layer; kekuatan antar-layer ±50–70 % bahan.
- Insert ulir panas (heat-set): lubang sesuai lembar data insert, biasanya
  Ø lubang ≈ Ø luar insert − 0,1…0,2 mm, dinding sekitar ≥ 1,5 × Ø insert.
