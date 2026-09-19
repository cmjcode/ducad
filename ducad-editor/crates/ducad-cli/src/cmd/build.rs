//! `ducad-cli build` (P10.2): part → artefak manufaktur + laporan, untuk CI.
//!
//! Deterministik: masukan dan `--date` sama → byte keluaran sama. Tanggal
//! default dari env `SOURCE_DATE_EPOCH`, baru kemudian tanggal hari ini.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ducad_engine::check::{CheckStatus, CheckSummary};
use ducad_engine::drawing_auto::{auto_sheet_model, hole_notes, TitleInfo};
use ducad_engine::export::{export, normalize_step_timestamp, ExportFormat};
use ducad_engine::ops::OpFile;
use ducad_engine::{OpErrorCode, Session};
use ducad_io::drawing::PaperSize;
use serde::Serialize;

use super::write_json;
use crate::{CliError, CliResult, Exit};

const ALL_FORMATS: [&str; 9] = ["step", "stl", "obj", "glb", "pdf", "svg", "dxf", "png", "bom"];
const BOM_HEADER: &str = "item,part,qty,material,volume_mm3,mass_g,file";
/// Batas baris tabel di `report.md` agar tetap ±60 baris.
const MD_MAX_ROWS: usize = 15;

#[derive(clap::Args)]
pub struct Args {
    /// Part `.ducad` atau OpFile `.json`.
    input: PathBuf,
    /// Folder keluaran (dibuat bila belum ada).
    #[arg(long)]
    out: PathBuf,
    /// Daftar format dipisah koma: step,stl,obj,glb,pdf,svg,dxf,png,bom.
    #[arg(long, value_delimiter = ',', default_value = "step,stl,pdf,png,bom")]
    formats: Vec<String>,
    /// Ukuran kertas gambar kerja: a4 | a3 (lanskap).
    #[arg(long, value_parser = parse_paper, default_value = "a3")]
    paper: PaperSize,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    part_number: Option<String>,
    #[arg(long, default_value = "")]
    author: String,
    #[arg(long, default_value = "A")]
    revision: String,
    /// `YYYY-MM-DD`; default dari `SOURCE_DATE_EPOCH` atau hari ini.
    #[arg(long, value_parser = parse_date)]
    date: Option<String>,
    /// Lewati checks desain.
    #[arg(long)]
    no_checks: bool,
}

fn parse_paper(s: &str) -> Result<PaperSize, String> {
    match s.to_ascii_lowercase().as_str() {
        "a4" => Ok(PaperSize::A4Landscape),
        "a3" => Ok(PaperSize::A3Landscape),
        "a2" => Err("kertas A2 belum didukung ducad-io (pakai a4 atau a3)".into()),
        other => Err(format!("kertas '{other}' tidak dikenal (a4|a3)")),
    }
}

fn parse_date(s: &str) -> Result<String, String> {
    let ok = s.len() == 10
        && s.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        });
    if ok {
        Ok(s.to_string())
    } else {
        Err(format!("tanggal '{s}' harus YYYY-MM-DD"))
    }
}

/// Hari sejak 1970-01-01 → (tahun, bulan, tanggal) kalender Gregorian.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn date_from_epoch(secs: i64) -> String {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

fn resolve_date(arg: Option<String>) -> Result<String, CliError> {
    if let Some(d) = arg {
        return Ok(d);
    }
    if let Ok(v) = std::env::var("SOURCE_DATE_EPOCH") {
        let secs: i64 = v
            .trim()
            .parse()
            .map_err(|_| CliError::usage(format!("SOURCE_DATE_EPOCH '{v}' bukan angka")))?;
        return Ok(date_from_epoch(secs));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(date_from_epoch(now))
}

#[derive(Serialize)]
struct BodyRow {
    name: String,
    volume_mm3: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    mass_g: Option<f64>,
    bbox: [[f64; 3]; 2],
}

#[derive(Serialize)]
struct Artifact {
    file: String,
    bytes: u64,
}

#[derive(Serialize)]
struct Report {
    input: String,
    date: String,
    /// `ops` | `replay` | `adopted`.
    load: &'static str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
    bodies: Vec<BodyRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checks: Option<CheckSummary>,
    artifacts: Vec<Artifact>,
    warnings: Vec<String>,
}

fn io_err(path: &Path, e: impl std::fmt::Display) -> CliError {
    CliError::usage(format!("gagal menulis {}: {e}", path.display()))
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn material_label(m: ducad_core::MaterialPreset) -> String {
    serde_json::to_value(m)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn mass_g(volume_mm3: f64, m: ducad_core::MaterialPreset) -> Option<f64> {
    m.density_g_cm3().map(|rho| volume_mm3 / 1000.0 * rho)
}

type VisibleBody<'a> = (&'a ducad_core::Body, &'a ducad_engine::model::BodyGeometry);

/// Body terlihat, terurut nama (urutan stabil untuk laporan dan BOM).
fn visible_bodies(s: &Session) -> Vec<VisibleBody<'_>> {
    let model = s.model();
    let mut v: Vec<_> = model
        .doc
        .bodies
        .iter()
        .filter(|(_, b)| b.visible)
        .filter_map(|(id, b)| Some((b, model.geometry.get(id)?)))
        .collect();
    v.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    v
}

fn body_rows(s: &Session) -> Vec<BodyRow> {
    let summary = s.summary();
    visible_bodies(s)
        .into_iter()
        .map(|(b, g)| {
            let volume = g.shape.volume();
            let bbox = summary
                .bodies
                .iter()
                .find(|r| r.name == b.name)
                .map(|r| r.bbox)
                .unwrap_or_default();
            BodyRow {
                name: b.name.clone(),
                volume_mm3: round2(volume),
                mass_g: mass_g(volume, b.material.preset).map(round2),
                bbox,
            }
        })
        .collect()
}

/// BOM CSV: satu baris per kelompok body dengan `mesh_fingerprint` sama.
fn bom_csv(s: &Session, file: &str) -> String {
    let mut groups: BTreeMap<u64, Vec<(&ducad_core::Body, f64)>> = BTreeMap::new();
    let mut order: Vec<u64> = Vec::new();
    for (b, g) in visible_bodies(s) {
        let key = g.mesh_fingerprint;
        if !groups.contains_key(&key) {
            order.push(key);
        }
        groups.entry(key).or_default().push((b, g.shape.volume()));
    }
    let csv = |t: &str| {
        if t.contains([',', '"', '\n']) {
            format!("\"{}\"", t.replace('"', "\"\""))
        } else {
            t.to_string()
        }
    };
    let mut out = String::from(BOM_HEADER);
    out.push('\n');
    for (i, key) in order.iter().enumerate() {
        let members = &groups[key];
        let (first, volume) = members[0];
        let preset = first.material.preset;
        let mass = mass_g(volume, preset)
            .map(|m| format!("{m:.2}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{:.2},{},{}\n",
            i + 1,
            csv(&first.name),
            members.len(),
            csv(&material_label(preset)),
            volume,
            mass,
            csv(file)
        ));
    }
    out
}

fn fmt_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "—".into(),
        serde_json::Value::Number(n) => n
            .as_f64()
            .map(|f| format!("{}", round2(f)))
            .unwrap_or_else(|| n.to_string()),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn report_md(r: &Report, title: &str) -> String {
    let mut md = format!("# DUCAD build — {title}\n\n");
    md.push_str(&format!(
        "Status: **{}** · masukan `{}` · muat: {} · tanggal {}\n",
        r.status, r.input, r.load, r.date
    ));
    if let Some(e) = &r.error {
        let msg = e.get("message").and_then(|m| m.as_str()).unwrap_or("");
        md.push_str(&format!("\n> Error: {msg}\n"));
    }
    if !r.bodies.is_empty() {
        md.push_str("\n## Body\n\n| Nama | Volume (mm³) | Massa (g) | Ukuran bbox (mm) |\n|---|---:|---:|---|\n");
        for b in r.bodies.iter().take(MD_MAX_ROWS) {
            let size: Vec<String> = (0..3)
                .map(|k| format!("{}", round2(b.bbox[1][k] - b.bbox[0][k])))
                .collect();
            let mass = b.mass_g.map(|m| format!("{m:.2}")).unwrap_or("—".into());
            md.push_str(&format!(
                "| {} | {:.2} | {} | {} |\n",
                b.name,
                b.volume_mm3,
                mass,
                size.join(" × ")
            ));
        }
        if r.bodies.len() > MD_MAX_ROWS {
            md.push_str(&format!("| … {} body lagi | | | |\n", r.bodies.len() - MD_MAX_ROWS));
        }
    }
    if let Some(c) = &r.checks {
        md.push_str(&format!(
            "\n## Checks — lulus {} · gagal {} · error {}\n\n| | Id | Terukur | Harapan |\n|---|---|---|---|\n",
            c.pass, c.fail, c.error
        ));
        for res in c.results.iter().take(MD_MAX_ROWS) {
            let icon = match res.status {
                CheckStatus::Pass => "✅",
                CheckStatus::Fail => "❌",
                CheckStatus::Error => "⚠️",
            };
            let id = res.id.clone().unwrap_or_else(|| res.kind.to_string());
            md.push_str(&format!(
                "| {icon} | {id} | {} | {} |\n",
                fmt_value(&res.measured),
                fmt_value(&res.expected)
            ));
        }
        if c.results.len() > MD_MAX_ROWS {
            md.push_str(&format!("| | … {} check lagi | | |\n", c.results.len() - MD_MAX_ROWS));
        }
    }
    if !r.artifacts.is_empty() {
        md.push_str("\n## Artefak\n\n");
        for a in &r.artifacts {
            md.push_str(&format!("- `{}` ({} B)\n", a.file, a.bytes));
        }
    }
    if !r.warnings.is_empty() {
        md.push_str("\n## Peringatan\n\n");
        for w in r.warnings.iter().take(5) {
            md.push_str(&format!("- {w}\n"));
        }
    }
    md
}

fn write_reports(out: &Path, r: &Report, title: &str) -> Result<(), CliError> {
    write_json(&out.join("report.json"), r)?;
    let p = out.join("report.md");
    std::fs::write(&p, report_md(r, title)).map_err(|e| io_err(&p, e))
}

type Loaded = (Result<Session, ducad_engine::OpError>, &'static str);

/// Muat masukan. Kegagalan op/replay dikembalikan sebagai `Err` di dalam
/// tuple (laporan tetap ditulis, kode 1); salah pakai → `CliError`.
fn load(input: &Path) -> Result<Loaded, CliError> {
    let is_json = input
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if !is_json {
        return Ok(match Session::from_file_strict(input) {
            Ok(s) if s.summary().warnings.iter().any(|w| w == "oplog_stale") => (Ok(s), "adopted"),
            Ok(s) => (Ok(s), "replay"),
            Err(mut e) if e.code == OpErrorCode::OplogStale => {
                e.message = format!("file tidak dapat direproduksi dari oplog: {}", e.message);
                (Err(e), "replay")
            }
            Err(e) => return Err(e.into()),
        });
    }
    let text = std::fs::read_to_string(input)
        .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", input.display())))?;
    let file: OpFile = serde_json::from_str(&text).map_err(|e| {
        CliError::usage(format!("{} bukan OpFile yang valid: {e}", input.display()))
    })?;
    let mut s = Session::new();
    let params = s.set_params(file.params)?;
    if let Some(e) = params.error {
        return Ok((Err(e), "ops"));
    }
    if !file.checks.is_empty() {
        s.set_checks(file.checks);
    }
    let report = s.run(file.ops, false);
    Ok(match report.error {
        Some(e) => (Err(e), "ops"),
        None => (Ok(s), "ops"),
    })
}

fn stem_of(input: &Path) -> String {
    let name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("part")
        .to_string();
    for suffix in [".ops.json", ".json", ".ducad"] {
        if let Some(s) = name.strip_suffix(suffix) {
            return s.to_string();
        }
    }
    name
}

pub fn exec(a: Args) -> CliResult {
    for f in &a.formats {
        if !ALL_FORMATS.contains(&f.as_str()) {
            return Err(CliError::usage(format!(
                "format '{f}' tidak dikenal ({})",
                ALL_FORMATS.join(",")
            )));
        }
    }
    let date = resolve_date(a.date)?;
    let stem = stem_of(&a.input);
    let title = a.title.clone().unwrap_or_else(|| stem.clone());
    std::fs::create_dir_all(&a.out).map_err(|e| io_err(&a.out, e))?;

    let input_label = a
        .input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let mut report = Report {
        input: input_label,
        date: date.clone(),
        load: "ops",
        status: "ok",
        error: None,
        bodies: Vec::new(),
        checks: None,
        artifacts: Vec::new(),
        warnings: Vec::new(),
    };

    let (loaded, mode) = load(&a.input)?;
    report.load = mode;
    let s = match loaded {
        Ok(s) => s,
        Err(e) => {
            report.status = "failed";
            report.error = serde_json::to_value(&e).ok();
            write_reports(&a.out, &report, &title)?;
            eprintln!("ducad-cli: {}", e.message);
            return Ok(Exit::OpFailed);
        }
    };
    report.warnings = s.summary().warnings;
    if mode == "adopted" {
        report
            .warnings
            .push("oplog basi: part diadopsi dari body berkas, bukan hasil replay".into());
    }
    report.bodies = body_rows(&s);

    if !a.no_checks && !s.design().checks.is_empty() {
        let summary = s.run_checks(None);
        let failed = summary.fail + summary.error > 0;
        report.checks = Some(summary);
        if failed {
            report.status = "checks_failed";
            write_reports(&a.out, &report, &title)?;
            return Ok(Exit::ChecksFailed);
        }
    }

    let has = |f: &str| a.formats.iter().any(|x| x == f);
    let mut written: Vec<PathBuf> = Vec::new();
    for (fmt, ext) in [
        (ExportFormat::Step, "step"),
        (ExportFormat::Stl, "stl"),
        (ExportFormat::Obj, "obj"),
        (ExportFormat::Glb, "glb"),
    ] {
        if !has(ext) {
            continue;
        }
        let p = a.out.join(format!("{stem}.{ext}"));
        export(&s, fmt, &p)?;
        if fmt == ExportFormat::Step {
            let text = std::fs::read_to_string(&p).map_err(|e| io_err(&p, e))?;
            std::fs::write(&p, normalize_step_timestamp(&text, &date))
                .map_err(|e| io_err(&p, e))?;
        }
        written.push(p);
    }

    if has("pdf") || has("svg") || has("dxf") {
        let material = visible_bodies(&s)
            .first()
            .map(|(b, _)| material_label(b.material.preset))
            .unwrap_or_default();
        let info = TitleInfo {
            title: title.clone(),
            part_number: a.part_number.clone().unwrap_or_else(|| stem.clone()),
            author: a.author.clone(),
            date: date.clone(),
            material,
            revision: a.revision.clone(),
        };
        let notes = hole_notes(s.design(), &s.design().params);
        let sheet = auto_sheet_model(s.model(), a.paper, &info, &notes)?;
        let draw_err = |p: &Path, e: anyhow::Error| io_err(p, format!("{e:#}"));
        if has("pdf") {
            let p = a.out.join(format!("{stem}-drawing.pdf"));
            ducad_io::pdf::export_pdf(&sheet, &p).map_err(|e| draw_err(&p, e))?;
            written.push(p);
        }
        if has("svg") {
            let p = a.out.join(format!("{stem}-drawing.svg"));
            ducad_io::svg::export_drawing_sheet_svg(&sheet, &p).map_err(|e| draw_err(&p, e))?;
            written.push(p);
        }
        if has("dxf") {
            let p = a.out.join(format!("{stem}-drawing.dxf"));
            ducad_io::dxf::export_drawing_sheet(&sheet, &p).map_err(|e| draw_err(&p, e))?;
            written.push(p);
        }
    }

    if has("png") {
        let p = a.out.join(format!("{stem}-iso.png"));
        super::render::write_view(
            &s,
            ducad_engine::render::View::Iso,
            true,
            &p,
            1200,
            900,
            false,
            None,
        )?;
        written.push(p);
    }

    if has("bom") {
        let p = a.out.join(format!("{stem}-bom.csv"));
        let file = if has("step") {
            format!("{stem}.step")
        } else {
            String::new()
        };
        std::fs::write(&p, bom_csv(&s, &file)).map_err(|e| io_err(&p, e))?;
        written.push(p);
    }

    for p in &written {
        let bytes = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let file = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        report.artifacts.push(Artifact { file, bytes });
    }
    write_reports(&a.out, &report, &title)?;
    Ok(Exit::Ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_to_date() {
        assert_eq!(date_from_epoch(0), "1970-01-01");
        assert_eq!(date_from_epoch(1_700_000_000), "2023-11-14");
        assert_eq!(date_from_epoch(951_782_400), "2000-02-29");
    }

    #[test]
    fn stem_strips_known_suffixes() {
        assert_eq!(stem_of(Path::new("a/plate.ops.json")), "plate");
        assert_eq!(stem_of(Path::new("b.ducad")), "b");
    }

    #[test]
    fn date_arg_validated() {
        assert!(parse_date("2026-01-02").is_ok());
        assert!(parse_date("2026-1-2").is_err());
        assert!(parse_paper("a2").is_err());
    }
}
