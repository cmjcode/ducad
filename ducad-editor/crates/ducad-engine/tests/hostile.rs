//! Input hostil tidak boleh membuat engine panik (prinsip "jangan pernah
//! crash", ADR 0009; pola tes `hostile_params_never_panic` di CADCraft).
//!
//! Setiap op pada fixture teruji dimutasi daun per daun (angka → 0/-1/
//! ±1e308/teks, string → kosong/panjang/selector rusak/ekspresi salah,
//! larik → kosong, …) lalu dijalankan `dry_run` di atas sesi yang sudah
//! memuat op-op sebelumnya. Hasil yang sah adalah `committed == false`
//! dengan error terstruktur apa pun KECUALI `internal` — kode itu hanya
//! muncul bila pagar `guard` menangkap panik, dan itulah yang dicari tes ini.

use ducad_engine::ops::{Op, OpFile};
use ducad_engine::{OpErrorCode, Session};
use serde_json::{json, Value};

const FIXTURES: &[(&str, &str)] = &[
    ("plate", include_str!("fixtures/plate.ops.json")),
    ("bracket", include_str!("fixtures/bracket.ops.json")),
];

/// Nilai pengganti yang "masuk akal secara tipe" supaya lolos serde dan
/// benar-benar sampai ke validasi/kernel, plus beberapa yang salah tipe.
fn hostile_values(orig: &Value) -> Vec<Value> {
    match orig {
        Value::Number(_) => vec![
            json!(0),
            json!(-1),
            json!(1e308),
            json!(-1e308),
            json!(1e-300),
            json!("$nope / 0"),
            json!(null),
        ],
        Value::String(_) => vec![
            json!(""),
            json!("x".repeat(300)),
            json!("$w/0"),
            json!("all[kind="),
            json!(">Z]]"),
            json!("(("),
            json!(0),
        ],
        Value::Bool(_) => vec![json!("yes"), json!(null)],
        Value::Array(_) => vec![json!([]), json!([null]), json!("x")],
        Value::Object(_) => vec![json!({}), json!(null)],
        Value::Null => vec![json!(0), json!("x")],
    }
}

/// Semua pointer JSON (kontainer dan daun) di dalam `v`.
fn pointers(v: &Value, prefix: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, c) in m {
                let p = format!("{prefix}/{k}");
                out.push(p.clone());
                pointers(c, &p, out);
            }
        }
        Value::Array(a) => {
            for (i, c) in a.iter().enumerate() {
                let p = format!("{prefix}/{i}");
                out.push(p.clone());
                pointers(c, &p, out);
            }
        }
        _ => {}
    }
}

fn assert_not_internal(label: &str, report: &ducad_engine::BatchReport) {
    if let Some(e) = &report.error {
        assert_ne!(
            e.code,
            OpErrorCode::Internal,
            "panik tertangkap pada {label}: {}",
            e.message
        );
    }
}

#[test]
fn mutated_fixture_ops_never_panic() {
    let mut tried = 0usize;
    for (name, text) in FIXTURES {
        let file: OpFile = serde_json::from_str(text).unwrap();
        let mut s = Session::new();
        assert!(s.set_params(file.params).unwrap().committed);
        for (i, op) in file.ops.iter().enumerate() {
            let orig = serde_json::to_value(op).unwrap();
            let mut ptrs = Vec::new();
            pointers(&orig, "", &mut ptrs);
            for p in ptrs
                .iter()
                .filter(|p| p.as_str() != "/op" && p.as_str() != "/id")
            {
                let Some(leaf) = orig.pointer(p) else {
                    continue;
                };
                for hv in hostile_values(leaf) {
                    let mut mutated = orig.clone();
                    if let Some(slot) = mutated.pointer_mut(p) {
                        *slot = hv;
                    }
                    // Mutasi yang ditolak serde bukan urusan engine.
                    let Ok(op) = serde_json::from_value::<Op>(mutated) else {
                        continue;
                    };
                    tried += 1;
                    let report = s.run(vec![op], true);
                    assert!(!report.committed, "dry_run tidak boleh commit");
                    assert_not_internal(&format!("{name} op#{i} {p}"), &report);
                }
            }
            let report = s.run(vec![op.clone()], false);
            assert!(report.committed, "{name} op#{i}: {:?}", report.error);
        }
    }
    assert!(tried > 100, "mutasi yang benar-benar dicoba: {tried}");
}

#[test]
fn hostile_params_and_edits_never_panic() {
    let file: OpFile = serde_json::from_str(FIXTURES[0].1).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(file.params).unwrap().committed);
    assert!(s.run(file.ops, false).committed);

    for raw in [
        json!({"w": "$w"}),
        json!({"w": "$w/0"}),
        json!({"w": "((("}),
        json!({"w": 1e308}),
        json!({"w": -1}),
        json!({"": 1}),
        json!({"x".repeat(200): "y".repeat(200)}),
    ] {
        let Ok(params) = serde_json::from_value::<ducad_engine::ops::Params>(raw) else {
            continue;
        };
        if let Ok(report) = s.set_params(params) {
            assert_not_internal("set_params", &report);
        }
    }

    for op in [
        json!({"op": "fillet", "id": "zz", "body": "", "edges": "", "radius": 0}),
        json!({"op": "fillet", "id": "zz", "body": "plate", "edges": "all[r=", "radius": 1e308}),
        json!({"op": "hole", "id": "zz", "body": "plate", "face": "", "at": [[1e308, -1e308]], "diameter": 1e-300}),
        json!({"op": "boolean", "id": "zz", "kind": "union", "a": "plate", "b": "plate"}),
        json!({"op": "pattern", "id": "zz", "body": "plate", "kind": "linear", "count": 0, "spacing": [0, 0, 0]}),
        json!({"op": "shell", "id": "zz", "body": "plate", "faces": ">Z", "thickness": -1e308}),
        json!({"op": "transform", "id": "zz", "body": "plate", "translate": [1e308, 1e308, 1e308]}),
    ] {
        let Ok(op) = serde_json::from_value::<Op>(op) else {
            continue;
        };
        let report = s.run(vec![op], true);
        assert!(!report.committed);
        assert_not_internal("op buatan", &report);
    }
}
