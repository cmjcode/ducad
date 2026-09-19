//! Evaluasi `CheckItem` terhadap geometri sesi (P7.1).

use serde::Serialize;
use serde_json::json;

use super::types::{BodySel, Check, CheckItem, CheckResult, CheckStatus};
use crate::inspect::round4;
use crate::model::{BodyGeometry, ModelDoc};
use crate::ops::{eval, eval_arr, Num, Params};
use crate::session::{SessionCore, SessionMeta};

/// Ringkasan hasil (dipakai CLI/MCP).
#[derive(Debug, Clone, Serialize)]
pub struct CheckSummary {
    pub pass: usize,
    pub fail: usize,
    pub error: usize,
    pub results: Vec<CheckResult>,
}

impl CheckSummary {
    pub fn from_results(results: Vec<CheckResult>) -> Self {
        let count = |s: CheckStatus| results.iter().filter(|r| r.status == s).count();
        Self {
            pass: count(CheckStatus::Pass),
            fail: count(CheckStatus::Fail),
            error: count(CheckStatus::Error),
            results,
        }
    }
}

pub fn all_pass(results: &[CheckResult]) -> bool {
    results.iter().all(|r| r.status == CheckStatus::Pass)
}

pub fn run_checks(core: &SessionCore, checks: &[CheckItem]) -> Vec<CheckResult> {
    run_checks_on(core.model, core.meta, checks)
}

/// Inti `run_checks` di atas state baca-saja.
pub fn run_checks_on(
    model: &ModelDoc,
    meta: &SessionMeta,
    checks: &[CheckItem],
) -> Vec<CheckResult> {
    let ctx = Ctx {
        model,
        params: &meta.design.params,
    };
    checks
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let r = ctx.run(&item.check).unwrap_or_else(Outcome::error);
            CheckResult {
                index: i,
                id: item.id.clone(),
                kind: item.check.kind(),
                status: r.status,
                measured: r.measured,
                expected: r.expected,
                message: r.message,
                location: r.location,
                body: r.body,
            }
        })
        .collect()
}

struct Outcome {
    status: CheckStatus,
    measured: serde_json::Value,
    expected: serde_json::Value,
    message: String,
    location: Option<[f64; 3]>,
    body: Option<String>,
}

impl Outcome {
    fn new(
        pass: bool,
        measured: serde_json::Value,
        expected: serde_json::Value,
        message: String,
    ) -> Self {
        Self {
            status: if pass {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            measured,
            expected,
            message,
            location: None,
            body: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            status: CheckStatus::Error,
            measured: serde_json::Value::Null,
            expected: serde_json::Value::Null,
            message,
            location: None,
            body: None,
        }
    }

    fn body(mut self, name: &str) -> Self {
        self.body = Some(name.to_string());
        self
    }

    fn at(mut self, p: Option<[f64; 3]>) -> Self {
        self.location = p;
        self
    }
}

type R = Result<Outcome, String>;

struct Ctx<'a> {
    model: &'a ModelDoc,
    params: &'a Params,
}

impl Ctx<'_> {
    fn num(&self, n: &Num) -> Result<f64, String> {
        eval(n, self.params).map_err(|e| e.message)
    }

    fn all_bodies(&self) -> Vec<(String, &BodyGeometry)> {
        let mut v: Vec<(String, &BodyGeometry)> = self
            .model
            .doc
            .bodies
            .iter()
            .filter_map(|(id, b)| Some((b.name.clone(), self.model.geometry.get(id)?)))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    fn by_name(&self, name: &str) -> Result<(String, &BodyGeometry), String> {
        self.all_bodies()
            .into_iter()
            .find(|(n, _)| n == name)
            .ok_or_else(|| format!("body '{name}' tidak dikenal"))
    }

    /// Field `body`: `"*"` = satu-satunya body.
    fn one(&self, sel: &BodySel) -> Result<(String, &BodyGeometry), String> {
        match sel {
            BodySel::One(s) if s == "*" => {
                let mut all = self.all_bodies();
                if all.len() != 1 {
                    return Err(format!("\"*\" butuh tepat 1 body, ada {}", all.len()));
                }
                Ok(all.remove(0))
            }
            BodySel::One(s) => self.by_name(s),
            BodySel::Many(v) if v.len() == 1 => self.by_name(&v[0]),
            BodySel::Many(v) => Err(format!("check ini butuh satu body, diberikan {}", v.len())),
        }
    }

    /// Field `bodies`: `"*"` = semua body.
    fn many(&self, sel: &BodySel) -> Result<Vec<(String, &BodyGeometry)>, String> {
        match sel {
            BodySel::One(s) if s == "*" => Ok(self.all_bodies()),
            BodySel::One(s) => Ok(vec![self.by_name(s)?]),
            BodySel::Many(v) => v.iter().map(|n| self.by_name(n)).collect(),
        }
    }

    fn run(&self, check: &Check) -> R {
        match check {
            Check::Valid { bodies } => {
                let bodies = self.many(bodies)?;
                if bodies.is_empty() {
                    return Ok(Outcome::new(
                        false,
                        json!([]),
                        json!("valid"),
                        "tidak ada body".into(),
                    ));
                }
                let invalid: Vec<&str> = bodies
                    .iter()
                    .filter(|(_, g)| !g.shape.is_valid())
                    .map(|(n, _)| n.as_str())
                    .collect();
                let msg = if invalid.is_empty() {
                    format!("{} body valid", bodies.len())
                } else {
                    format!("body tidak valid: {}", invalid.join(", "))
                };
                Ok(Outcome::new(
                    invalid.is_empty(),
                    json!({ "invalid": invalid }),
                    json!("valid"),
                    msg,
                ))
            }
            Check::BodyCount { expect } => {
                let n = self.all_bodies().len();
                Ok(Outcome::new(
                    n == *expect,
                    json!(n),
                    json!(expect),
                    format!("jumlah body {n}, diharapkan {expect}"),
                ))
            }
            Check::Volume {
                body,
                expect,
                min,
                max,
                tol_pct,
            } => {
                let (name, g) = self.one(body)?;
                let v = g.shape.volume().abs();
                let (pass, expected, msg) = match (expect, min, max) {
                    (Some(e), _, _) => {
                        let e = self.num(e)?;
                        if e.abs() < 1e-12 {
                            return Err("expect volume tidak boleh 0".into());
                        }
                        let dev = (v - e).abs() / e.abs() * 100.0;
                        (
                            dev <= *tol_pct,
                            json!({ "expect": e, "tol_pct": tol_pct }),
                            format!("volume {v:.3} mm³, diharapkan {e:.3} ± {tol_pct}% (selisih {dev:.3}%)"),
                        )
                    }
                    (None, lo, hi) => {
                        let lo = lo.as_ref().map(|n| self.num(n)).transpose()?;
                        let hi = hi.as_ref().map(|n| self.num(n)).transpose()?;
                        if lo.is_none() && hi.is_none() {
                            return Err("volume butuh expect atau min/max".into());
                        }
                        (
                            lo.is_none_or(|l| v >= l) && hi.is_none_or(|h| v <= h),
                            json!({ "min": lo, "max": hi }),
                            format!("volume {v:.3} mm³, batas [{lo:?}, {hi:?}]"),
                        )
                    }
                };
                Ok(Outcome::new(pass, json!(round4(v)), expected, msg).body(&name))
            }
            Check::BboxSize { body, expect, tol } => {
                let (name, g) = self.one(body)?;
                let size = bbox_size(g)?;
                let e = eval_arr(expect, self.params).map_err(|e| e.message)?;
                let pass = (0..3).all(|i| (size[i] - e[i]).abs() <= *tol);
                Ok(Outcome::new(
                    pass,
                    json!(size.map(round4)),
                    json!({ "expect": e, "tol": tol }),
                    format!(
                        "ukuran bbox {:?}, diharapkan {e:?} ± {tol}",
                        size.map(round4)
                    ),
                )
                .body(&name))
            }
            Check::BboxMax { body, max } => {
                let (name, g) = self.one(body)?;
                let size = bbox_size(g)?;
                let m = eval_arr(max, self.params).map_err(|e| e.message)?;
                let pass = (0..3).all(|i| size[i] <= m[i] + 1e-3);
                Ok(Outcome::new(
                    pass,
                    json!(size.map(round4)),
                    json!({ "max": m }),
                    format!("ukuran bbox {:?}, maksimum {m:?}", size.map(round4)),
                )
                .body(&name))
            }
            Check::Mass {
                body,
                min_g,
                max_g,
                density_g_cm3,
            } => {
                let (name, g) = self.one(body)?;
                let density = match density_g_cm3 {
                    Some(d) => *d,
                    None => {
                        let preset = self
                            .model
                            .doc
                            .bodies
                            .values()
                            .find(|b| b.name == name)
                            .map(|b| b.material.preset);
                        preset
                            .and_then(|p| p.density_g_cm3())
                            .ok_or("densitas material tidak diketahui; isi density_g_cm3")?
                    }
                };
                let grams = g.shape.volume().abs() / 1000.0 * density;
                let lo = min_g.as_ref().map(|n| self.num(n)).transpose()?;
                let hi = max_g.as_ref().map(|n| self.num(n)).transpose()?;
                let pass = lo.is_none_or(|l| grams >= l) && hi.is_none_or(|h| grams <= h);
                Ok(Outcome::new(
                    pass,
                    json!(round4(grams)),
                    json!({ "min_g": lo, "max_g": hi, "density_g_cm3": density }),
                    format!(
                        "massa {grams:.2} g (densitas {density} g/cm³), batas [{lo:?}, {hi:?}]"
                    ),
                )
                .body(&name))
            }
            Check::MinWall { body, min } => {
                let (name, g) = self.one(body)?;
                let min = self.num(min)?;
                let report =
                    ducad_kernel::min_wall_thickness(&g.mesh, ducad_kernel::DEFAULT_WALL_SAMPLES)
                        .ok_or("tebal dinding tidak bisa diukur (mesh tanpa sampel)")?;
                let t = report.min as f64;
                let pass = t >= min - 0.05;
                Ok(Outcome::new(
                    pass,
                    json!({ "min": round4(t), "p05": round4(report.p05 as f64), "samples": report.samples }),
                    json!({ "min": min }),
                    format!("tebal dinding minimum {t:.3} mm (p05 {:.3}), diharapkan ≥ {min}", report.p05),
                )
                .body(&name)
                .at(Some(report.at.map(|v| round4(v as f64)))))
            }
            Check::HoleCount {
                body,
                diameter,
                tol,
                expect,
            } => {
                let (name, g) = self.one(body)?;
                let d = self.num(diameter)?;
                let faces = ducad_kernel::enumerate_faces(&g.shape);
                let holes = super::holes::find_holes(&faces, d, *tol);
                let n = holes.len();
                let location = if n > *expect {
                    holes.get(*expect).map(|h| h.point.map(round4))
                } else {
                    None
                };
                Ok(Outcome::new(
                    n == *expect,
                    json!(n),
                    json!({ "expect": expect, "diameter": d, "tol": tol }),
                    format!("{n} lubang Ø{d} ± {tol}, diharapkan {expect}"),
                )
                .body(&name)
                .at(location))
            }
            Check::Clearance { a, b, min } => {
                let (_, ga) = self.by_name(a)?;
                let (_, gb) = self.by_name(b)?;
                let min = self.num(min)?;
                let c = ducad_kernel::check_clearance(&ga.shape, &gb.shape, min)
                    .map_err(|e| format!("{e:#}"))?;
                let mid = [
                    (c.point_a.0 + c.point_b.0) * 0.5,
                    (c.point_a.1 + c.point_b.1) * 0.5,
                    (c.point_a.2 + c.point_b.2) * 0.5,
                ];
                Ok(Outcome::new(
                    c.passes,
                    json!(round4(c.distance)),
                    json!({ "min": min }),
                    format!("celah {a}–{b} {:.4} mm, minimum {min}", c.distance),
                )
                .at(Some(mid.map(round4))))
            }
            Check::NoInterference { bodies, tol_mm3 } => {
                let bodies = self.many(bodies)?;
                let list: Vec<(u64, String, &ducad_kernel::KernelShape)> = bodies
                    .iter()
                    .enumerate()
                    .map(|(i, (n, g))| (i as u64, n.clone(), &g.shape))
                    .collect();
                let clashes = ducad_kernel::detect_interference(&list, *tol_mm3);
                let pairs: Vec<serde_json::Value> = clashes
                    .iter()
                    .map(|c| json!({ "a": c.body_a_name, "b": c.body_b_name, "volume": round4(c.volume) }))
                    .collect();
                let location = clashes.first().map(|c| c.center.map(round4));
                let msg = if clashes.is_empty() {
                    format!("{} body tanpa interferensi", bodies.len())
                } else {
                    format!("{} pasangan body saling tembus", clashes.len())
                };
                Ok(Outcome::new(
                    clashes.is_empty(),
                    json!(pairs),
                    json!({ "tol_mm3": tol_mm3 }),
                    msg,
                )
                .at(location))
            }
        }
    }
}

fn bbox_size(g: &BodyGeometry) -> Result<[f64; 3], String> {
    let (min, max) = g.mesh.bounding_box().ok_or("body tanpa mesh")?;
    Ok([0, 1, 2].map(|i| (max[i] - min[i]) as f64))
}
