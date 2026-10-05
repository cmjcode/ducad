//! DXF pola bentangan sheet metal (P19): garis luar di layer `OUTLINE`,
//! garis tekuk di `BEND_UP` / `BEND_DOWN` — konvensi yang dipakai mesin
//! potong laser dan operator press brake.

use ducad_core::FlatPattern;

pub const LAYER_OUTLINE: &str = "OUTLINE";
pub const LAYER_BEND_UP: &str = "BEND_UP";
pub const LAYER_BEND_DOWN: &str = "BEND_DOWN";

fn push_line(out: &mut String, layer: &str, a: [f64; 2], b: [f64; 2]) {
    out.push_str(&format!(
        "0\nLINE\n8\n{layer}\n10\n{}\n20\n{}\n30\n0.0\n11\n{}\n21\n{}\n31\n0.0\n",
        a[0], a[1], b[0], b[1]
    ));
}

/// Teks DXF (AC1009) pola datar. Deterministik: urutan entitas = urutan
/// `flat.outline` lalu `flat.bend_lines`.
pub fn flat_pattern_dxf(flat: &FlatPattern) -> String {
    let mut out = String::new();
    out.push_str("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n");
    out.push_str("0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n3\n");
    // Warna ACI: 7 putih/hitam, 3 hijau, 1 merah.
    for (layer, color) in [(LAYER_OUTLINE, 7), (LAYER_BEND_UP, 3), (LAYER_BEND_DOWN, 1)] {
        out.push_str(&format!(
            "0\nLAYER\n2\n{layer}\n70\n0\n62\n{color}\n6\nCONTINUOUS\n"
        ));
    }
    out.push_str("0\nENDTAB\n0\nENDSEC\n");
    out.push_str("0\nSECTION\n2\nENTITIES\n");
    for (a, b) in &flat.outline {
        push_line(&mut out, LAYER_OUTLINE, *a, *b);
    }
    for bend in &flat.bend_lines {
        let layer = if bend.up {
            LAYER_BEND_UP
        } else {
            LAYER_BEND_DOWN
        };
        push_line(&mut out, layer, bend.a, bend.b);
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::{BendSegment, BendTable, Flange, ReliefKind, SheetMetalModel};

    fn model() -> SheetMetalModel {
        let flange = |edge: usize, angle: f64| Flange {
            id: format!("f{edge}"),
            edge,
            segments: vec![BendSegment {
                angle_deg: angle,
                radius: 2.0,
                length: 20.0,
            }],
            relief: ReliefKind::None,
        };
        SheetMetalModel {
            outline: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 60.0], [0.0, 60.0]],
            thickness: 2.0,
            default_radius: 2.0,
            k_factor: 0.44,
            flanges: vec![flange(0, 90.0), flange(2, -90.0)],
            bend_table: BendTable::default(),
        }
    }

    #[test]
    fn dxf_flat_pattern_has_bend_layers_and_reimports_fully() {
        let flat = model().flat_pattern().unwrap();
        let dxf = flat_pattern_dxf(&flat);
        for layer in [LAYER_OUTLINE, LAYER_BEND_UP, LAYER_BEND_DOWN] {
            assert!(
                dxf.contains(&format!("\n8\n{layer}\n")),
                "layer {layer} tidak ada"
            );
        }
        // 2 sisi dasar bebas + 2 strip × 3 sisi + 2 garis tekuk.
        let written = flat.outline.len() + flat.bend_lines.len();
        assert_eq!(written, 2 + 6 + 2);
        assert_eq!(
            dxf.matches("\n0\nLINE\n").count() + usize::from(dxf.starts_with("0\nLINE\n")),
            written
        );

        // Importer DXF sendiri membaca semuanya kembali, tanpa yang terlewat.
        let back = crate::dxf::import_str(&dxf).unwrap();
        assert_eq!(back.skipped, 0);
        assert_eq!(back.entities.len(), written);
        assert!(back
            .entities
            .iter()
            .all(|e| matches!(e, ducad_sketch::Entity::Line { .. })));
        // Deterministik.
        assert_eq!(dxf, flat_pattern_dxf(&flat));
    }
}
