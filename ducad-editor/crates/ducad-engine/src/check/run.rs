//! Evaluasi `CheckItem` terhadap geometri sesi (P7.1).

use serde::Serialize;
use serde_json::json;

use super::types::{
    BodySel, Check, CheckItem, CheckResult, CheckStatus, InertiaAxis, StackMethod,
};
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
        params: meta.design.effective_params(),
        meta,
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
    params: Params,
    meta: &'a SessionMeta,
}

impl Ctx<'_> {
    /// Model sheet metal body `name`.
    fn sheet(&self, name: &str) -> Result<&ducad_core::SheetMetalModel, String> {
        self.meta
            .sheet_metal
            .get(name)
            .map(|s| &s.model)
            .ok_or_else(|| format!("body '{name}' bukan part sheet metal (buat dengan op base_flange)"))
    }

    /// Id studi: `"*"` = satu-satunya studi di oplog yang jenisnya lolos
    /// `wanted` (mis. hanya studi frekuensi untuk `min_natural_frequency`).
    fn study_id(
        &self,
        study: &str,
        what: &str,
        wanted: impl Fn(crate::ops::StudyKind) -> bool,
    ) -> Result<String, String> {
        if study != "*" {
            return Ok(study.to_string());
        }
        let all: Vec<String> = crate::sim::study_defs(self.meta)
            .into_iter()
            .filter(|(_, d)| wanted(d.kind))
            .map(|(id, _)| id.to_string())
            .collect();
        match all.as_slice() {
            [only] => Ok(only.clone()),
            [] => Err(format!("tidak ada studi {what} di oplog")),
            many => Err(format!(
                "\"*\" butuh tepat satu studi {what}, ada {}; sebut id-nya",
                many.len()
            )),
        }
    }

    /// Hasil segar studi tegangan `study` (`static` / `thermal_stress`).
    fn study(&self, study: &str) -> Result<(String, &ducad_sim::SimReport), String> {
        let id = self.study_id(study, "tegangan", |k| k.has_stress())?;
        let report = crate::sim::fresh_result(self.model, self.meta, &id)?;
        Ok((id, report))
    }

    /// Hasil segar studi frekuensi/buckling/termal berjenis `kind`.
    fn analysis(
        &self,
        study: &str,
        kind: crate::ops::StudyKind,
    ) -> Result<(String, &crate::sim::AnalysisReport), String> {
        let id = self.study_id(study, kind.name(), |k| k == kind)?;
        let report = crate::sim::fresh_analysis(self.model, self.meta, &id)?;
        Ok((id, report))
    }

    /// Densitas body (g/cm³): nilai eksplisit check, lalu material mekanik,
    /// lalu perkiraan preset visual.
    fn density(&self, name: &str, explicit: Option<f64>) -> Result<f64, String> {
        if let Some(d) = explicit {
            return Ok(d);
        }
        self.model
            .doc
            .bodies
            .values()
            .find(|b| b.name == name)
            .and_then(|b| self.model.doc.density_of(b))
            .ok_or_else(|| "densitas material tidak diketahui; isi density_g_cm3".to_string())
    }

    fn num(&self, n: &Num) -> Result<f64, String> {
        eval(n, &self.params).map_err(|e| e.message)
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
                let e = eval_arr(expect, &self.params).map_err(|e| e.message)?;
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
                let m = eval_arr(max, &self.params).map_err(|e| e.message)?;
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
                let density = self.density(&name, *density_g_cm3)?;
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
            Check::CenterOfMass { body, expect, tol } => {
                let (name, g) = self.one(body)?;
                let want = [
                    self.num(&expect[0])?,
                    self.num(&expect[1])?,
                    self.num(&expect[2])?,
                ];
                let mp = g.shape.mass_properties();
                if mp.volume_mm3 <= 1e-9 {
                    return Err("body tidak bervolume; pusat massa tidak terdefinisi".into());
                }
                let got = mp.centroid;
                let pass = got.iter().zip(want).all(|(a, b)| (a - b).abs() <= *tol);
                Ok(Outcome::new(
                    pass,
                    json!(got.map(round4)),
                    json!({ "expect": want, "tol": tol }),
                    format!(
                        "pusat massa [{:.3}, {:.3}, {:.3}] mm, diharapkan [{}, {}, {}] ± {tol}",
                        got[0], got[1], got[2], want[0], want[1], want[2]
                    ),
                )
                .body(&name)
                .at(Some(got.map(round4))))
            }
            Check::MomentOfInertia {
                body,
                axis,
                min,
                max,
                density_g_cm3,
            } => {
                let (name, g) = self.one(body)?;
                let density = self.density(&name, *density_g_cm3)?;
                let mp = g.shape.mass_properties();
                let rho = density / 1000.0;
                let com = mp.inertia_com();
                let (principal, _) = mp.principal();
                let value = rho
                    * match axis {
                        InertiaAxis::X => com[0][0],
                        InertiaAxis::Y => com[1][1],
                        InertiaAxis::Z => com[2][2],
                        InertiaAxis::PrincipalMin => principal[0],
                        InertiaAxis::PrincipalMid => principal[1],
                        InertiaAxis::PrincipalMax => principal[2],
                    };
                let lo = min.as_ref().map(|n| self.num(n)).transpose()?;
                let hi = max.as_ref().map(|n| self.num(n)).transpose()?;
                let pass = lo.is_none_or(|l| value >= l) && hi.is_none_or(|h| value <= h);
                Ok(Outcome::new(
                    pass,
                    json!(round4(value)),
                    json!({ "axis": axis, "min": lo, "max": hi, "density_g_cm3": density }),
                    format!(
                        "momen inersia {value:.2} g·mm² (densitas {density} g/cm³), batas [{lo:?}, {hi:?}]"
                    ),
                )
                .body(&name))
            }
            Check::MaxStress { study, max_mpa } => {
                let (id, r) = self.study(study)?;
                let limit = self.num(max_mpa)?;
                let v = r.max_von_mises_mpa;
                Ok(Outcome::new(
                    v <= limit,
                    json!(round4(v)),
                    json!({ "study": id, "max_mpa": limit }),
                    format!("von Mises maksimum {v:.3} MPa, batas {limit} MPa (studi '{id}')"),
                )
                .at(Some(r.location.map(round4))))
            }
            Check::MaxDisplacement { study, max_mm } => {
                let (id, r) = self.study(study)?;
                let limit = self.num(max_mm)?;
                let v = r.max_displacement_mm;
                Ok(Outcome::new(
                    v <= limit,
                    json!((v * 1e6).round() / 1e6),
                    json!({ "study": id, "max_mm": limit }),
                    format!("deformasi maksimum {v:.5} mm, batas {limit} mm (studi '{id}')"),
                ))
            }
            Check::MinSafetyFactor { study, min } => {
                let (id, r) = self.study(study)?;
                let limit = self.num(min)?;
                let v = r.safety_factor;
                Ok(Outcome::new(
                    v >= limit,
                    json!(round4(v)),
                    json!({ "study": id, "min": limit }),
                    format!("faktor keamanan {v:.3}, minimum {limit} (studi '{id}')"),
                )
                .at(Some(r.location.map(round4))))
            }
            Check::MinNaturalFrequency { study, min_hz } => {
                let (id, r) = self.analysis(study, crate::ops::StudyKind::Frequency)?;
                let limit = self.num(min_hz)?;
                let crate::sim::AnalysisReport::Frequency(r) = r else {
                    return Err(format!("studi '{id}' bukan studi frekuensi"));
                };
                let v = r
                    .frequencies_hz
                    .first()
                    .copied()
                    .ok_or_else(|| format!("studi '{id}' tidak menghasilkan mode"))?;
                Ok(Outcome::new(
                    v >= limit,
                    json!(round4(v)),
                    json!({ "study": id, "min_hz": limit }),
                    format!("frekuensi natural pertama {v:.3} Hz, minimum {limit} Hz (studi '{id}')"),
                ))
            }
            Check::MinBucklingFactor { study, min } => {
                let (id, r) = self.analysis(study, crate::ops::StudyKind::Buckling)?;
                let limit = self.num(min)?;
                let crate::sim::AnalysisReport::Buckling(r) = r else {
                    return Err(format!("studi '{id}' bukan studi buckling"));
                };
                // Tanpa faktor = beban tidak menimbulkan tekuk → lulus.
                Ok(match r.load_factors.first().copied() {
                    Some(v) => Outcome::new(
                        v >= limit,
                        json!(round4(v)),
                        json!({ "study": id, "min": limit }),
                        format!("faktor tekuk kritis {v:.3}, minimum {limit} (studi '{id}')"),
                    ),
                    None => Outcome::new(
                        true,
                        serde_json::Value::Null,
                        json!({ "study": id, "min": limit }),
                        format!("beban studi '{id}' tidak menimbulkan tekuk"),
                    ),
                })
            }
            Check::MaxTemperature { study, max_c } => {
                let (id, r) = self.analysis(study, crate::ops::StudyKind::Thermal)?;
                let limit = self.num(max_c)?;
                let crate::sim::AnalysisReport::Thermal(r) = r else {
                    return Err(format!("studi '{id}' bukan studi termal"));
                };
                let v = r.max_temperature_c;
                Ok(Outcome::new(
                    v <= limit,
                    json!(round4(v)),
                    json!({ "study": id, "max_c": limit }),
                    format!("suhu maksimum {v:.2} °C, batas {limit} °C (studi '{id}')"),
                )
                .at(Some(r.location.map(round4))))
            }
            Check::MinBendRadius {
                body,
                min_ratio_to_t,
            } => {
                let (name, _) = self.one(body)?;
                let limit = self.num(min_ratio_to_t)?;
                let model = self.sheet(&name)?;
                let Some(ratio) = model.min_bend_ratio() else {
                    return Ok(Outcome::new(
                        true,
                        serde_json::Value::Null,
                        json!({ "min_ratio_to_t": limit }),
                        "part pelat tanpa tekukan".to_string(),
                    )
                    .body(&name));
                };
                Ok(Outcome::new(
                    ratio + 1e-9 >= limit,
                    json!(round4(ratio)),
                    json!({ "min_ratio_to_t": limit, "thickness": model.thickness }),
                    format!(
                        "radius tekuk terkecil {:.3} mm = {ratio:.3} × tebal {} mm, minimum {limit} × tebal",
                        ratio * model.thickness,
                        model.thickness
                    ),
                )
                .body(&name))
            }
            Check::MinFlangeLength { body, min } => {
                let (name, _) = self.one(body)?;
                let limit = self.num(min)?;
                let model = self.sheet(&name)?;
                let Some(shortest) = model.min_flange_length() else {
                    return Ok(Outcome::new(
                        true,
                        serde_json::Value::Null,
                        json!({ "min": limit }),
                        "part pelat tanpa flange".to_string(),
                    )
                    .body(&name));
                };
                Ok(Outcome::new(
                    shortest + 1e-9 >= limit,
                    json!(round4(shortest)),
                    json!({ "min": limit }),
                    format!("flange terpendek {shortest:.3} mm, minimum {limit} mm"),
                )
                .body(&name))
            }
            Check::ToleranceStackup {
                chain,
                max_total,
                method,
            } => {
                let limit = self.num(max_total)?;
                if chain.is_empty() {
                    return Err("rantai toleransi kosong".into());
                }
                let mut links = Vec::with_capacity(chain.len());
                for (i, link) in chain.iter().enumerate() {
                    let nominal = self.num(&link.nominal)?;
                    let (plus, minus) = match (&link.fit, &link.plus, &link.minus) {
                        (Some(fit), None, None) => {
                            let fit = ducad_core::IsoFit::parse(fit)
                                .map_err(|e| format!("mata rantai {i}: {e}"))?;
                            let (upper, lower) = ducad_core::iso286::limits(nominal.abs(), &fit)
                                .map_err(|e| format!("mata rantai {i}: {e}"))?;
                            (upper, -lower)
                        }
                        (None, plus, minus) => (
                            plus.as_ref().map(|n| self.num(n)).transpose()?.unwrap_or(0.0),
                            minus.as_ref().map(|n| self.num(n)).transpose()?.unwrap_or(0.0),
                        ),
                        _ => {
                            return Err(format!(
                                "mata rantai {i}: isi `fit` ATAU `plus`/`minus`, jangan keduanya"
                            ))
                        }
                    };
                    // Mata rantai berlawanan arah: nominal dikurangkan, batas bertukar.
                    links.push(if link.reverse {
                        (-nominal, minus, plus)
                    } else {
                        (nominal, plus, minus)
                    });
                }
                let r = ducad_core::drawing_annot::tolerance_stackup(&links);
                let total = match method {
                    StackMethod::WorstCase => r.worst_case_total(),
                    StackMethod::Rss => r.rss_total(),
                };
                Ok(Outcome::new(
                    total <= limit + 1e-12,
                    json!({
                        "nominal": round4(r.nominal),
                        "total": round4(total),
                        "worst_case": [round4(r.worst_case_plus), round4(r.worst_case_minus)],
                        "rss": [round4(r.rss_plus), round4(r.rss_minus)],
                    }),
                    json!({ "max_total": limit, "method": method }),
                    format!(
                        "tumpukan toleransi {total:.4} mm pada nominal {:.4} mm, batas {limit} mm",
                        r.nominal
                    ),
                ))
            }
            Check::MinWall { body, min } => {
                let (name, g) = self.one(body)?;
                let min = self.num(min)?;
                let r = evaluate_min_wall(&g.mesh, &name, min);
                Ok(Outcome {
                    status: r.status,
                    measured: r.measured,
                    expected: r.expected,
                    message: r.message,
                    location: r.location,
                    body: r.body,
                })
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

/// Resolusi check `min_wall` ke (nama body, batas minimum) tanpa mengukur —
/// dipakai GUI untuk menjalankan [`evaluate_min_wall`] di thread latar.
pub fn resolve_min_wall(
    model: &ModelDoc,
    meta: &SessionMeta,
    item: &CheckItem,
) -> Option<(String, f64)> {
    let Check::MinWall { body, min } = &item.check else {
        return None;
    };
    let ctx = Ctx {
        model,
        params: meta.design.effective_params(),
        meta,
    };
    let (name, _) = ctx.one(body).ok()?;
    Some((name, ctx.num(min).ok()?))
}

/// Evaluasi `min_wall` langsung dari mesh. Tidak menyentuh OCCT, jadi aman
/// di thread latar. `index`/`id` hasil diisi pemanggil.
pub fn evaluate_min_wall(mesh: &ducad_kernel::KernelMesh, body: &str, min: f64) -> CheckResult {
    let base = CheckResult {
        index: 0,
        id: None,
        kind: "min_wall",
        status: CheckStatus::Error,
        measured: serde_json::Value::Null,
        expected: json!({ "min": min }),
        message: "tebal dinding tidak bisa diukur (mesh tanpa sampel)".into(),
        location: None,
        body: Some(body.to_string()),
    };
    let Some(report) = ducad_kernel::min_wall_thickness(mesh, ducad_kernel::DEFAULT_WALL_SAMPLES)
    else {
        return base;
    };
    let t = report.min as f64;
    CheckResult {
        status: if t >= min - 0.05 {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        measured: json!({ "min": round4(t), "p05": round4(report.p05 as f64), "samples": report.samples }),
        message: format!(
            "tebal dinding minimum {t:.3} mm (p05 {:.3}), diharapkan ≥ {min}",
            report.p05
        ),
        location: Some(report.at.map(|v| round4(v as f64))),
        ..base
    }
}
