//! Serialisasi oplog ramah git (P8.1): `{ "params", "ops", "checks" }` dengan
//! SATU op/check per baris, sehingga diff git menunjuk op yang berubah.

use serde::Serialize;

use crate::session::DesignDoc;

fn compact(v: &impl Serialize) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

fn write_array<T: Serialize>(out: &mut String, key: &str, items: &[T], last: bool) {
    out.push_str(&format!("  \"{key}\": ["));
    if items.is_empty() {
        out.push(']');
    } else {
        out.push('\n');
        for (i, item) in items.iter().enumerate() {
            out.push_str("    ");
            out.push_str(&compact(item));
            if i + 1 < items.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ]");
    }
    out.push_str(if last { "\n" } else { ",\n" });
}

/// Teks `*.ops.json` untuk `design`: params (kunci terurut, `BTreeMap`),
/// lalu satu op per baris, lalu satu check per baris. Hasilnya JSON valid
/// yang bisa dibaca balik sebagai `OpFile` (mis. oleh `ducad-cli run`).
pub fn to_git_text(design: &DesignDoc) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"params\": {},\n", compact(&design.params)));
    write_array(&mut out, "ops", &design.oplog, false);
    // `drawings` hanya ditulis bila ada, supaya berkas lama tidak berubah.
    let has_drawings = !design.drawings.is_empty();
    write_array(&mut out, "checks", &design.checks, !has_drawings);
    if has_drawings {
        write_array(&mut out, "drawings", &design.drawings, true);
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_op_per_line_and_valid_json() {
        let f: crate::ops::OpFile = serde_json::from_str(crate::ops::EXAMPLE_PLATE).unwrap();
        let design = DesignDoc {
            params: f.params.clone(),
            oplog: f.ops.clone(),
            ..DesignDoc::default()
        };
        let text = to_git_text(&design);
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["ops"].as_array().unwrap().len(), 4);
        let op_lines = text
            .lines()
            .filter(|l| l.trim_start().starts_with("{\"op\":"))
            .count();
        assert_eq!(op_lines, 4);
        assert!(
            text.contains("\"params\": {\"h\":40.0,\"r\":3.0,\"t\":8.0,\"w\":60.0}"),
            "{text}"
        );
        assert!(text.contains("\"checks\": []"));
        let back: crate::ops::OpFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back.ops, f.ops);
    }
}
