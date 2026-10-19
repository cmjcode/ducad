//! Migrasi berkas `.ducad` ke format v4 (rencana migrasi kernel, F1.3).
//!
//! Body yang terbukti bisa direproduksi dari oplog `design` dilepas dari
//! STEP-nya dan diganti cache B-rep native berlabel produsen. Body tanpa
//! oplog (GUI/impor), berkas basi, atau oplog yang gagal di-replay
//! dibiarkan apa adanya — STEP-nya tetap otoritatif. Hasil migrasi selalu
//! diverifikasi: dimuat ulang dan sidik jarinya harus sama dengan berkas
//! asal; gagal → error, berkas tidak boleh ditulis.
//!
//! Field lain (sketch tampilan, tinta, round history, uuid, material)
//! dipertahankan byte-demi-byte lewat struktur `DuCADFile` mentah.

use serde::Serialize;

use ducad_io::native::{NativeBrepCache, FORMAT_VERSION_DERIVED};

use crate::error::{OpError, OpErrorCode, OpResult};
use crate::session::{fingerprint, Session};

/// Toleransi relatif volume body berkas vs hasil replay.
// Volume dari tesselasi konvergen, ±1e-4 relatif (PLAN_MIGRASI_TRUCK §3.1),
// juga untuk body STEP yang dibaca.
const VOLUME_TOL: f64 = 1e-4;

/// Satu body yang tetap membawa STEP, beserta alasannya.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KeptBody {
    pub name: String,
    pub reason: String,
}

/// Laporan migrasi satu berkas.
#[derive(Debug, Clone, Serialize)]
pub struct MigrateReport {
    pub input_version: u32,
    pub output_version: u32,
    /// `false` = keluaran identik dengan masukan.
    pub changed: bool,
    /// Body yang kini turunan oplog (tanpa STEP, dengan cache).
    pub derived: Vec<String>,
    /// Body yang tetap membawa STEP.
    pub kept_step: Vec<KeptBody>,
    /// Alasan tingkat berkas bila tidak ada yang dimigrasi.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    pub bytes_before: usize,
    pub bytes_after: usize,
    /// Sidik jari geometri (sama sebelum & sesudah).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub fingerprint: String,
}

fn io(msg: impl Into<String>) -> OpError {
    OpError::new(OpErrorCode::Io, msg)
}

/// Migrasikan isi berkas `.ducad` ke format v4. Mengembalikan teks baru +
/// laporan; bila tidak ada body yang bisa dimigrasi, teks = masukan.
pub fn migrate_json(json: &str) -> OpResult<(String, MigrateReport)> {
    let mut file = ducad_io::native::deserialize_raw(json)
        .map_err(|e| io(format!("berkas tidak valid: {e:#}")))?;
    let mut report = MigrateReport {
        input_version: file.format_version,
        output_version: file.format_version,
        changed: false,
        derived: Vec::new(),
        kept_step: Vec::new(),
        skipped: None,
        bytes_before: json.len(),
        bytes_after: json.len(),
        fingerprint: String::new(),
    };
    let unchanged = |mut report: MigrateReport, why: String| {
        report.kept_step = file_body_names(json)
            .into_iter()
            .map(|name| KeptBody {
                name,
                reason: why.clone(),
            })
            .collect();
        report.skipped = Some(why);
        Ok((json.to_string(), report))
    };

    if file.design.is_none() {
        return unchanged(report, "no_oplog".into());
    }
    // Strict: berkas yang oplog-nya tidak mereproduksi geometri tidak boleh
    // kehilangan STEP-nya.
    let session = match Session::from_json_strict(json) {
        Ok(s) => s,
        Err(e) => return unchanged(report, format!("replay_failed: {}", e.message)),
    };
    if !session.meta().warnings.is_empty() {
        let why = format!("not_replayable: {}", session.meta().warnings.join(", "));
        return unchanged(report, why);
    }
    let expected = fingerprint(session.model());
    report.fingerprint = expected.clone();

    let mut stripped = 0usize;
    for body in &mut file.bodies {
        if body.step.is_none() {
            report.derived.push(body.name.clone());
            continue;
        }
        let keep = |reason: &str, report: &mut MigrateReport| {
            report.kept_step.push(KeptBody {
                name: body.name.clone(),
                reason: reason.to_string(),
            })
        };
        let Ok((_, geo)) = session.body(&body.name) else {
            keep("not_produced_by_oplog", &mut report);
            continue;
        };
        // STEP rusak tapi replay sehat (strict) → tetap dimigrasi: oplog
        // terbukti mereproduksi sidik jari berkas.
        let mut warnings = Vec::new();
        if let Ok(Some(original)) = body.resolve_shape(&mut warnings) {
            let (v0, v1) = (original.volume().abs(), geo.shape.volume().abs());
            if (v0 - v1).abs() > VOLUME_TOL * v0.max(1.0) {
                keep(&format!("volume_mismatch: {v0} vs {v1}"), &mut report);
                continue;
            }
        }
        let cache = NativeBrepCache::of(&geo.shape)
            .map_err(|e| io(format!("gagal encode cache body '{}': {e:#}", body.name)))?;
        body.step = None;
        body.brep = Some(cache);
        report.derived.push(body.name.clone());
        stripped += 1;
    }

    if stripped == 0 {
        return Ok((json.to_string(), MigrateReport {
            skipped: Some("nothing_to_migrate".into()),
            ..report
        }));
    }
    file.format_version = FORMAT_VERSION_DERIVED;
    let out = serde_json::to_string_pretty(&file)
        .map_err(|e| io(format!("gagal serialisasi berkas: {e}")))?;

    // Verifikasi: hasil migrasi harus memuat ulang ke geometri yang sama,
    // baik lewat replay (engine) maupun lewat cache (GUI).
    let reloaded = Session::from_json_strict(&out)
        .map_err(|e| io(format!("verifikasi gagal: berkas hasil tidak bisa di-replay: {}", e.message)))?;
    let actual = fingerprint(reloaded.model());
    if actual != expected {
        return Err(io(format!(
            "verifikasi gagal: sidik jari berubah ({expected} → {actual})"
        )));
    }
    let gui = ducad_io::native::deserialize_from_json(&out)
        .map_err(|e| io(format!("verifikasi gagal: pemuat GUI menolak hasil: {e:#}")))?;
    if gui.needs_replay() {
        return Err(io("verifikasi gagal: cache brep hasil migrasi tidak terbaca"));
    }

    report.output_version = FORMAT_VERSION_DERIVED;
    report.changed = true;
    report.bytes_after = out.len();
    Ok((out, report))
}

fn file_body_names(json: &str) -> Vec<String> {
    ducad_io::native::deserialize_raw(json)
        .map(|f| f.bodies.into_iter().map(|b| b.name).collect())
        .unwrap_or_default()
}
