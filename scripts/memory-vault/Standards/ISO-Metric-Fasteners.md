---
tags: [ducad, standard]
---
# Pengencang Metrik ISO (M2–M12)

Nilai persis yang dipakai engine DUCAD (`IsoMetricThread::standard_params`,
`ducad-core/src/hole.rs`). Semua dalam mm. Pitch = ulir kasar (coarse).
Clearance = lubang pas baut normal; counterbore mengikuti ISO 4762 (baut L),
countersink mengikuti ISO 10642.

<!-- tabel:mulai -->
| Ukuran | Pitch | Tap drill Ø | Clearance Ø | Counterbore Ø | Counterbore dalam | Countersink Ø |
|---|---|---|---|---|---|---|
| M2 | 0.40 | 1.60 | 2.4 | 4.4 | 2.4 | 4.4 |
| M2.5 | 0.45 | 2.05 | 2.9 | 5.0 | 2.9 | 5.5 |
| M3 | 0.50 | 2.50 | 3.4 | 6.5 | 3.4 | 6.7 |
| M4 | 0.70 | 3.30 | 4.5 | 8.0 | 4.4 | 8.9 |
| M5 | 0.80 | 4.20 | 5.5 | 10.0 | 5.4 | 11.2 |
| M6 | 1.00 | 5.00 | 6.6 | 11.5 | 6.5 | 13.4 |
| M8 | 1.25 | 6.80 | 9.0 | 15.0 | 8.6 | 17.9 |
| M10 | 1.50 | 8.50 | 11.0 | 18.0 | 10.6 | 22.4 |
| M12 | 1.75 | 10.20 | 13.5 | 20.0 | 12.6 | 26.8 |
<!-- tabel:akhir -->

Pemetaan ke `Op::Hole`: `{"iso":"M5"}` = clearance (Ø di kolom Clearance),
`{"iso":"M5","kind":"tapped"}` = tap drill, `counterbore`/`countersink`
memakai kolom yang sesuai. `depth` kosong = tembus.
