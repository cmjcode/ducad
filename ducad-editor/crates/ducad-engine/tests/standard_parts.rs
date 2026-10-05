//! Toolbox part standar + ulir (P20).

use ducad_core::{standard_part, StandardKind, StandardShape};
use ducad_engine::inspect::{summarize, DEFAULT_TOPOLOGY_LIMIT};
use ducad_engine::ops::Op;
use ducad_engine::{OpErrorCode, Session};

fn ops(json: &str) -> Vec<Op> {
    serde_json::from_str(json).unwrap()
}

const PI: f64 = std::f64::consts::PI;

/// Luas heksagon beraturan dari lebar kunci.
fn hex_area(across_flats: f64) -> f64 {
    3.0_f64.sqrt() / 2.0 * across_flats * across_flats
}

#[test]
fn standard_parts_every_entry_is_valid_with_table_dimensions() {
    let mut checked = 0;
    for kind in StandardKind::ALL {
        for size in kind.sizes() {
            let length = kind.needs_length().then_some(20.0);
            let part = standard_part(kind, size, length).unwrap();
            let length_json = length
                .map(|l| format!(r#","length":{l}"#))
                .unwrap_or_default();
            let mut s = Session::new();
            let r = s.run(
                ops(&format!(
                    r#"[{{"op":"standard_part","id":"p","standard":"{}","size":"{size}"{length_json}}}]"#,
                    kind.key()
                )),
                false,
            );
            assert!(r.committed, "{} {size}: {:?}", kind.key(), r.error);
            let sum = summarize(&s, None, false, DEFAULT_TOPOLOGY_LIMIT).unwrap();
            let b = &sum.bodies[0];
            assert!(b.valid, "{} {size}", part.designation);
            assert_eq!(b.standard.as_deref(), Some(part.designation.as_str()));
            // Dimensi kunci + volume analitik.
            let (dx, dz, volume) = match part.shape {
                StandardShape::CapScrew {
                    d,
                    length,
                    head_d,
                    head_h,
                    socket_af,
                    socket_depth,
                } => (
                    head_d,
                    length + head_h,
                    PI / 4.0 * (d * d * length + head_d * head_d * head_h)
                        - hex_area(socket_af) * socket_depth,
                ),
                StandardShape::HexBolt {
                    d,
                    length,
                    across_flats,
                    head_h,
                } => (
                    across_flats * 2.0 / 3.0_f64.sqrt(),
                    length + head_h,
                    PI / 4.0 * d * d * length + hex_area(across_flats) * head_h,
                ),
                StandardShape::HexNut {
                    d,
                    across_flats,
                    height,
                } => (
                    across_flats * 2.0 / 3.0_f64.sqrt(),
                    height,
                    (hex_area(across_flats) - PI / 4.0 * d * d) * height,
                ),
                StandardShape::Ring {
                    inner_d,
                    outer_d,
                    height,
                } => (
                    outer_d,
                    height,
                    PI / 4.0 * (outer_d * outer_d - inner_d * inner_d) * height,
                ),
                StandardShape::Pin { d, length } => (d, length, PI / 4.0 * d * d * length),
            };
            assert!(
                (b.size[0] - dx).abs() < 1e-3,
                "{}: lebar {} vs {dx}",
                part.designation,
                b.size[0]
            );
            assert!(
                (b.size[2] - dz).abs() < 1e-3,
                "{}: tinggi {} vs {dz}",
                part.designation,
                b.size[2]
            );
            assert!(
                (b.volume - volume).abs() / volume < 1e-4,
                "{}: volume {} vs {volume}",
                part.designation,
                b.volume
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 7 * 4 + 8 + 9);
}

#[test]
fn standard_parts_placement_and_errors() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"standard_part","id":"bolt","standard":"ISO 4762","size":"M6","length":20,"at":[10,5,8]}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(r.outcomes[0].detail["designation"], "ISO 4762 - M6 x 20");
    let b = &summarize(&s, None, false, 10).unwrap().bodies[0];
    // Bidang bawah kepala di z = 8: batang turun 20, kepala naik 6.
    assert!(
        (b.bbox[0][2] + 12.0).abs() < 1e-6 && (b.bbox[1][2] - 14.0).abs() < 1e-6,
        "{:?}",
        b.bbox
    );
    assert!((b.bbox[0][0] - 5.0).abs() < 1e-3 && (b.bbox[1][0] - 15.0).abs() < 1e-3);

    for (bad, what) in [
        (
            r#"{"op":"standard_part","id":"x","standard":"din912","size":"M6","length":10}"#,
            "standar",
        ),
        (
            r#"{"op":"standard_part","id":"x","standard":"iso4032","size":"M7"}"#,
            "ukuran",
        ),
        (
            r#"{"op":"standard_part","id":"x","standard":"iso4014","size":"M6"}"#,
            "length",
        ),
        (
            r#"{"op":"standard_part","id":"x","standard":"bearing","size":"6204","length":3}"#,
            "length",
        ),
    ] {
        let r = s.run(ops(&format!("[{bad}]")), false);
        let e = r.error.unwrap();
        assert_eq!(e.code, OpErrorCode::InvalidParam, "{what}");
        assert!(e.message.contains(what), "{}", e.message);
    }
}

#[test]
fn standard_parts_thread_cosmetic_and_physical() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[
            {"op":"primitive","id":"rod","shape":{"cylinder":{"r":5,"h":30}}},
            {"op":"thread","id":"note","body":"rod","face":"all[kind=cylinder]"}
        ]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let plain = s.body("rod").unwrap().1.shape.volume().abs();
    assert!(
        (plain - PI * 25.0 * 30.0).abs() < 1e-6,
        "ulir kosmetik tidak mengubah geometri"
    );
    let b = &summarize(&s, None, false, 10).unwrap().bodies[0];
    assert_eq!(b.threads.len(), 1);
    assert_eq!(b.threads[0].designation, "M10x1.5");
    assert!(b.threads[0].cosmetic && b.threads[0].length == 30.0);

    // Ulir fisik 12 mm dari ujung atas, kisar eksplisit.
    let r = s.run(
        ops(
            r#"[{"op":"thread","id":"cut","body":"rod","face":"all[kind=cylinder]","length":12,
                 "pitch":1.5,"from_end":true,"cosmetic":false}]"#,
        ),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let removed = plain - s.body("rod").unwrap().1.shape.volume().abs();
    let expected = ducad_kernel::IsoThreadProfile::new(1.5).removed_volume(10.0, 12.0);
    assert!(
        (removed - expected).abs() / expected < 0.03,
        "{removed} vs {expected}"
    );
    let b = &summarize(&s, None, false, 10).unwrap().bodies[0];
    assert!(b.valid);
    assert_eq!(b.threads.len(), 2);
    // Alur ada di separuh atas: pusat massa turun.
    assert!(b.center_of_mass.unwrap()[2] < 15.0);

    // Diameter non-standar tanpa `pitch`, face non-silinder, panjang berlebih.
    let mut t = Session::new();
    assert!(
        t.run(
            ops(
                r#"[{"op":"primitive","id":"odd","shape":{"cylinder":{"r":7.3,"h":10}}},
                    {"op":"primitive","id":"blk","shape":{"box":{"size":[5,5,5]}}}]"#
            ),
            false
        )
        .committed
    );
    for bad in [
        r#"{"op":"thread","id":"t","body":"odd","face":"all[kind=cylinder]"}"#,
        r#"{"op":"thread","id":"t","body":"blk","face":">Z"}"#,
        r#"{"op":"thread","id":"t","body":"odd","face":"all[kind=cylinder]","pitch":1,"length":50}"#,
        r#"{"op":"thread","id":"t","body":"odd","face":"all","pitch":1}"#,
    ] {
        let r = t.run(ops(&format!("[{bad}]")), false);
        assert_eq!(r.error.unwrap().code, OpErrorCode::InvalidParam, "{bad}");
    }
}
