//! `sketch_project`: lengkapi error dengan `hint` + konteks yang menuntun
//! agent ke perbaikan (bidang tidak memotong body, selector kosong, dst.).
//! Tidak ada fix otomatis: penyebabnya hampir selalu pilihan bidang/selector.

use serde_json::json;

use crate::error::{OpError, OpErrorCode};
use crate::ops::{Op, ProjectMode};
use crate::session::SessionCore;

pub(super) fn diagnose(core: &mut SessionCore, op: &Op, mut err: OpError) -> OpError {
    let Op::SketchProject {
        sketch, body, mode, ..
    } = op
    else {
        return err;
    };
    if err.hint.is_some() {
        return err;
    }
    let hint = match (err.code, mode) {
        (OpErrorCode::EmptyResult, ProjectMode::Intersect) => {
            // Sertakan jarak bidang ke bbox body supaya agent tahu harus geser ke mana.
            if let (Ok((_, frame)), Ok((_, geo))) = (core.sketch(sketch), core.body(body)) {
                if let Some((lo, hi)) = geo.mesh.bounding_box() {
                    let n = frame.normal_v();
                    let o = frame.origin_v();
                    let (mut dmin, mut dmax) = (f64::MAX, f64::MIN);
                    for x in [lo[0], hi[0]] {
                        for y in [lo[1], hi[1]] {
                            for z in [lo[2], hi[2]] {
                                let d = (glam::DVec3::new(x as f64, y as f64, z as f64) - o).dot(n);
                                dmin = dmin.min(d);
                                dmax = dmax.max(d);
                            }
                        }
                    }
                    let ctx = json!({ "body_span_along_normal": [dmin, dmax] });
                    match err.context.as_object_mut() {
                        Some(m) => {
                            m.insert("body_span_along_normal".into(), ctx["body_span_along_normal"].clone());
                        }
                        None => err.context = ctx,
                    }
                }
            }
            "bidang sketch tidak memotong body: geser offset bidang ke dalam rentang body_span_along_normal (0 = bidang sketch)"
        }
        (OpErrorCode::EmptyResult, ProjectMode::Project) => {
            "semua tepi terpilih tegak lurus bidang sketch (runtuh jadi titik): pilih tepi/face lain, mis. faces \">Z\" untuk sketch di bidang XY"
        }
        (OpErrorCode::SelectorEmpty, _) | (OpErrorCode::SelectorSyntax, _) => {
            "uji selector dengan query_geometry dulu; faces memakai selector face (\">Z\"), edges memakai selector tepi (\"of(>Z)\", \"|X\")"
        }
        (OpErrorCode::UnknownRef, _) => {
            "`sketch` harus id op sketch yang sudah ada dan `body` nama body yang masih hidup (lihat inspect)"
        }
        (OpErrorCode::DuplicateId, _) => {
            "entitas baru bernama <id>.1, <id>.2, …: ganti id op sketch_project agar tidak bentrok dengan nama entitas sketch"
        }
        (OpErrorCode::BodyConsumed, _) => "body sudah dilebur boolean: pakai id hasil boolean",
        _ => return err,
    };
    err.with_hint(hint)
}
