# Baseline waktu op per contoh — backend occt

Dihasilkan oleh `ducad-cli golden capture --perf` (build debug kecuali disebut lain).
Waktu satu op mencakup hitung kernel + tessellasi + dimensi tepi (`compute::finish`).
Data mentah: berkas `.json` di sebelah dokumen ini.

## Per contoh

| Contoh | Total (ms) | Op | Op terlambat |
|---|---:|---:|---|
| `../examples/design-package/plate_review.ops.json` | 149.7 | 5 | `h1` hole (75.1 ms) |
| `../examples/design-package/sensor_bracket.ops.json` | 57.2 | 7 | `sensor_holes` hole (27.2 ms) |
| `../examples/design-package/tray.ops.json` | 177.1 | 5 | `wall_y` edge_flange (112.8 ms) |
| `../examples/industrial-demo/model.ducad` | 18.3 | 0 | — (muat `.ducad`) |
| `../examples/industrial-demo/mounting_bracket.ducad` | 783.5 | 0 | — (muat `.ducad`) |
| `../examples/industrial-demo/mounting_bracket.ops.json` | 998.5 | 24 | `bolt_b` standard_part (96.9 ms) |
| `../examples/pillow_block.ops.json` | 1272.9 | 36 | `vibration` study (171.2 ms) |
| `../examples/vector_logo.ops.json` | 2.6 | 1 | `vector_sketch` sketch (0.1 ms) |
| `crates/ducad-engine/tests/fixtures/bracket.ops.json` | 99.2 | 6 | `holes` hole (48.8 ms) |
| `crates/ducad-engine/tests/fixtures/enclosure.ops.json` | 287.4 | 6 | `vents` hole (107.1 ms) |
| `crates/ducad-engine/tests/fixtures/flange.ops.json` | 285.4 | 6 | `part` boolean (104.6 ms) |
| `crates/ducad-engine/tests/fixtures/loft_helix.ops.json` | 8208.2 | 5 | `spring` helix (7129.5 ms) |
| `crates/ducad-engine/tests/fixtures/mass_bracket.ops.json` | 26.1 | 7 | `bracket` boolean (6.3 ms) |
| `crates/ducad-engine/tests/fixtures/plate.ops.json` | 62.7 | 4 | `h1` hole (43.3 ms) |
| `crates/ducad-engine/tests/fixtures/sheet_box.ops.json` | 108.8 | 5 | `wall_y` edge_flange (54.1 ms) |
| `crates/ducad-engine/tests/fixtures/sim_bracket.ops.json` | 13.5 | 5 | `bracket` boolean (5.8 ms) |
| `crates/ducad-engine/tests/fixtures/sim_modes.ops.json` | 7.0 | 6 | `beam` primitive (1.4 ms) |
| `crates/ducad-engine/tests/fixtures/toolbox_joint.ops.json` | 72.4 | 6 | `bolt` standard_part (21.6 ms) |
| `crates/ducad-engine/tests/fixtures/valve_body.ops.json` | 963.8 | 15 | `bolts_top` hole (233.3 ms) |
| `evals/fixtures/plate_param.ops.json` | 59.3 | 4 | `h1` hole (41.1 ms) |
| `examples/vector_logo.ops.json` | 13.2 | 2 | `logo3d` extrude (12.3 ms) |

## Per jenis op

| Op | Jumlah | Rata-rata (ms) | Maks (ms) | Total (ms) |
|---|---:|---:|---:|---:|
| `helix` | 1 | 7129.5 | 7129.5 | 7129.5 |
| `mirror` | 1 | 983.4 | 983.4 | 983.4 |
| `hole` | 12 | 81.2 | 233.3 | 973.9 |
| `study` | 13 | 37.8 | 171.2 | 491.8 |
| `set_material` | 21 | 22.0 | 64.9 | 461.4 |
| `standard_part` | 12 | 38.3 | 96.9 | 459.5 |
| `edge_flange` | 6 | 58.1 | 112.8 | 348.6 |
| `boolean` | 10 | 25.1 | 104.6 | 251.4 |
| `fillet` | 9 | 27.6 | 88.1 | 248.5 |
| `transform` | 9 | 23.2 | 98.1 | 208.7 |
| `revolve` | 4 | 46.8 | 85.0 | 187.2 |
| `extrude` | 7 | 19.5 | 66.4 | 136.5 |
| `chamfer` | 3 | 40.3 | 58.5 | 120.8 |
| `primitive` | 20 | 5.3 | 25.5 | 105.3 |
| `loft` | 1 | 94.5 | 94.5 | 94.5 |
| `pattern` | 1 | 92.0 | 92.0 | 92.0 |
| `shell` | 1 | 74.6 | 74.6 | 74.6 |
| `sketch` | 17 | 4.0 | 16.5 | 68.8 |
| `thread` | 3 | 18.4 | 23.4 | 55.1 |
| `flat_pattern` | 1 | 27.5 | 27.5 | 27.5 |
| `base_flange` | 3 | 8.5 | 20.6 | 25.6 |
