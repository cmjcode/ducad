//! Tampilan oplog untuk Feature Tree (Step 2 `docs/PLAN_DESIGNER_UX.md`):
//! nama, jenis, parameter utama, status, dan dependensi tiap op — dibaca
//! langsung dari `DesignDoc`, sumber kebenaran pemodelan (ADR 0010).
//!
//! Dependensi diturunkan dari rujukan id: setiap nilai string di op yang
//! sama dengan id op sebelumnya (nama body/sketch, `axis:<id>`,
//! `point:<id>`, atau body turunan `<id>.<n>`) berarti op itu bergantung
//! pada op tersebut.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::ops::{Num, Op};
use crate::session::DesignDoc;

/// Field angka yang dianggap "nilai utama" op, urut prioritas.
const MAIN_KEYS: &[&str] = &[
    "distance",
    "radius",
    "thickness",
    "angle_deg",
    "angle",
    "depth",
    "offset",
];

/// Satu baris Feature Tree dari oplog.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpRow {
    pub id: String,
    /// Jenis op (`extrude`, `fillet`, …).
    pub kind: String,
    /// Ringkasan singkat parameter utama, mis. `"60 mm"`, `"R3"`.
    pub summary: String,
    /// `(field, nilai)` angka literal yang bisa diedit langsung; `None` bila
    /// op tidak punya nilai utama atau nilainya ekspresi param.
    pub main_value: Option<(String, f64)>,
    /// Ekspresi param nilai utama (mis. `"$t"`), bila ada.
    pub main_expr: Option<String>,
    pub suppressed: bool,
    /// Op yang dirujuk op ini.
    pub depends_on: Vec<String>,
    /// Op yang merujuk op ini.
    pub dependents: Vec<String>,
}

fn strings(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        serde_json::Value::Object(m) => m
            .iter()
            .filter(|(k, _)| k.as_str() != "id" && k.as_str() != "op")
            .for_each(|(_, x)| strings(x, out)),
        _ => {}
    }
}

/// Id op yang dirujuk `s` (nama apa adanya, `axis:`/`point:`, atau body
/// turunan `id.n`).
fn referenced_id<'a>(s: &'a str, known: &BTreeSet<&str>) -> Option<&'a str> {
    let s = s
        .strip_prefix("axis:")
        .or_else(|| s.strip_prefix("point:"))
        .unwrap_or(s);
    if known.contains(s) {
        return Some(s);
    }
    let (base, suffix) = s.rsplit_once('.')?;
    (suffix.chars().all(|c| c.is_ascii_digit()) && known.contains(base)).then_some(base)
}

fn main_value(v: &serde_json::Value) -> (Option<(String, f64)>, Option<String>) {
    for key in MAIN_KEYS {
        match v.get(*key) {
            Some(serde_json::Value::Number(n)) => {
                if let Some(x) = n.as_f64() {
                    return (Some(((*key).to_string(), x)), None);
                }
            }
            Some(serde_json::Value::String(e)) => return (None, Some(e.clone())),
            _ => {}
        }
    }
    (None, None)
}

fn fmt(x: f64) -> String {
    let s = format!("{x:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn summary(
    kind: &str,
    v: &serde_json::Value,
    main: &Option<(String, f64)>,
    expr: &Option<String>,
) -> String {
    let val = main.as_ref().map(|(_, x)| fmt(*x)).or_else(|| expr.clone());
    let target = v
        .get("body")
        .or_else(|| v.get("sketch"))
        .and_then(|b| b.as_str());
    let mut out = match (kind, &val) {
        ("fillet", Some(x)) => format!("R{x}"),
        ("chamfer", Some(x)) | ("extrude", Some(x)) | ("shell", Some(x)) => format!("{x} mm"),
        ("revolve", Some(x)) => format!("{x} deg"),
        ("hole", _) => v
            .get("spec")
            .map(|s| {
                s.get("iso")
                    .and_then(|i| i.as_str())
                    .map(str::to_string)
                    .or_else(|| s.get("diameter").map(|d| format!("D{d}")))
                    .unwrap_or_default()
            })
            .unwrap_or_default(),
        ("sketch", _) => {
            let n = v
                .get("entities")
                .and_then(|e| e.as_array())
                .map_or(0, Vec::len);
            let plane = v.get("plane").and_then(|p| p.as_str()).unwrap_or("");
            format!("{plane}, {n} entitas")
        }
        (_, Some(x)) => x.clone(),
        _ => String::new(),
    };
    if let Some(t) = target {
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(t);
    }
    out
}

/// Baris Feature Tree untuk seluruh oplog `design`.
pub fn oplog_rows(design: &DesignDoc) -> Vec<OpRow> {
    let mut rows: Vec<OpRow> = Vec::with_capacity(design.oplog.len());
    let mut known: BTreeSet<&str> = BTreeSet::new();
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for op in &design.oplog {
        let v = serde_json::to_value(op).unwrap_or_default();
        let mut refs = Vec::new();
        strings(&v, &mut refs);
        let mut depends_on: Vec<String> = refs
            .iter()
            .filter_map(|s| referenced_id(s, &known))
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        depends_on.retain(|d| d != op.id());
        for d in &depends_on {
            dependents
                .entry(d.clone())
                .or_default()
                .push(op.id().to_string());
        }
        let kind = op.kind().to_string();
        let (main, expr) = main_value(&v);
        rows.push(OpRow {
            id: op.id().to_string(),
            summary: summary(&kind, &v, &main, &expr),
            kind,
            main_value: main,
            main_expr: expr,
            suppressed: design.is_suppressed(op.id()),
            depends_on,
            dependents: Vec::new(),
        });
        known.insert(op.id());
    }
    for row in &mut rows {
        if let Some(d) = dependents.remove(&row.id) {
            row.dependents = d;
        }
    }
    rows
}

/// Salinan `op` dengan field angka `key` diganti `value` (untuk edit nilai
/// utama dari Feature Tree). `None` bila field tidak ada.
pub fn with_value(op: &Op, key: &str, value: f64) -> Option<Op> {
    let mut v = serde_json::to_value(op).ok()?;
    let slot = v.get_mut(key)?;
    *slot = serde_json::to_value(Num::Value(value)).ok()?;
    serde_json::from_value(v).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn design() -> DesignDoc {
        let f: crate::ops::OpFile =
            serde_json::from_str(include_str!("../tests/fixtures/sample_bracket.ops.json"))
                .unwrap();
        DesignDoc {
            params: f.params,
            oplog: f.ops,
            ..DesignDoc::default()
        }
    }

    #[test]
    fn rows_have_kinds_summaries_and_dependencies() {
        let rows = oplog_rows(&design());
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "profile",
                "bracket",
                "inner_fillet",
                "base_holes",
                "wall_holes"
            ]
        );
        let by = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert_eq!(by("bracket").depends_on, ["profile"]);
        assert_eq!(by("inner_fillet").depends_on, ["bracket"]);
        assert_eq!(
            by("bracket").dependents,
            ["inner_fillet", "base_holes", "wall_holes"]
        );
        assert_eq!(by("profile").summary, "XZ, 6 entitas");
        assert_eq!(by("base_holes").summary, "M5, bracket");
        // Nilai berupa ekspresi param tidak bisa diedit langsung.
        assert_eq!(by("inner_fillet").main_value, None);
        assert_eq!(by("inner_fillet").main_expr.as_deref(), Some("$r"));
    }

    #[test]
    fn with_value_patches_literal_and_suppression_is_reported() {
        let mut d = design();
        let fillet: Op = serde_json::from_value(serde_json::json!({
            "op":"fillet","id":"f","body":"b","edges":"|Z","radius":2
        }))
        .unwrap();
        let patched = with_value(&fillet, "radius", 4.5).unwrap();
        let v = serde_json::to_value(&patched).unwrap();
        assert_eq!(v["radius"], 4.5);
        assert!(with_value(&fillet, "distance", 1.0).is_none());

        d.suppressed.push("inner_fillet".into());
        let rows = oplog_rows(&d);
        assert!(
            rows.iter()
                .find(|r| r.id == "inner_fillet")
                .unwrap()
                .suppressed
        );
    }
}
