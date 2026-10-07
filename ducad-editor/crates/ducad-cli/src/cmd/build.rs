//! `ducad-cli build` (P10.2): part → artefak manufaktur + laporan, untuk CI.
//!
//! Deterministik: masukan dan `--date` sama → byte keluaran sama. Tanggal
//! default dari env `SOURCE_DATE_EPOCH`, baru kemudian tanggal hari ini.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ducad_engine::check::{CheckStatus, CheckSummary};
use ducad_engine::drawing_auto::{build_sheet, hole_notes};
use ducad_io::drawing::{ScaleSpec, SectionSpec, ShadedSpec};
use ducad_engine::export::{export, normalize_step_timestamp, ExportFormat};
use ducad_engine::ops::OpFile;
use ducad_engine::{OpErrorCode, Session};
use ducad_io::drawing::PaperSize;
use serde::Serialize;

use super::write_json;
use crate::{CliError, CliResult, Exit};

const ALL_FORMATS: [&str; 10] = [
    "step", "stl", "obj", "glb", "pdf", "svg", "dxf", "png", "bom", "flat",
];
const BOM_HEADER: &str = "item,part,qty,material,volume_mm3,mass_g,file,standard";
/// Batas baris tabel di `report.md` agar tetap ±60 baris.
const MD_MAX_ROWS: usize = 15;

#[derive(clap::Args)]
pub struct Args {
    /// Part `.ducad` atau OpFile `.json`.
    input: PathBuf,
    /// Folder keluaran (dibuat bila belum ada).
    #[arg(long)]
    out: PathBuf,
    /// Daftar format dipisah koma: step,stl,obj,glb,pdf,svg,dxf,png,bom,flat
    /// (`flat` = DXF pola bentangan tiap body sheet metal).
    #[arg(long, value_delimiter = ',', default_value = "step,stl,pdf,png,bom")]
    formats: Vec<String>,
    /// Ukuran kertas gambar kerja: a4 | a3 (lanskap). Default: kertas lembar
    /// tersimpan, atau a3.
    #[arg(long, value_parser = parse_paper)]
    paper: Option<PaperSize>,
    /// Lembar gambar tersimpan yang dirender (default: yang pertama).
    #[arg(long)]
    drawing: Option<String>,
    /// Potongan `LABEL:INDUK:SUMBU:OFFSET[:flip]`, mis. `A:top:x:0`. Boleh
    /// diulang; menggantikan potongan lembar tersimpan.
    #[arg(long = "section", value_parser = parse_section)]
    sections: Vec<SectionSpec>,
    /// Render berbayang dipisah koma: iso,iso_back.
    #[arg(long, value_delimiter = ',', value_parser = parse_shaded)]
    shaded: Vec<ShadedSpec>,
    /// Skala lembar: auto | 1:2 | 0.5.
    #[arg(long, value_parser = parse_sheet_scale)]
    scale: Option<ScaleSpec>,
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
    /// Bangun satu konfigurasi varian (nama; "Default" = desain dasar).
    #[arg(long, conflicts_with = "all_configs")]
    config: Option<String>,
    /// Bangun SEMUA konfigurasi, masing-masing ke subfolder bernama
    /// konfigurasi itu, plus ringkasan matriks di folder keluaran.
    #[arg(long)]
    all_configs: bool,
}

fn parse_paper(s: &str) -> Result<PaperSize, String> {
    match s.to_ascii_lowercase().as_str() {
        "a4" => Ok(PaperSize::A4Landscape),
        "a3" => Ok(PaperSize::A3Landscape),
        "a2" => Err("kertas A2 belum didukung ducad-io (pakai a4 atau a3)".into()),
        other => Err(format!("kertas '{other}' tidak dikenal (a4|a3)")),
    }
}

fn parse_section(s: &str) -> Result<SectionSpec, String> {
    let parts: Vec<&str> = s.split(':').collect();
    if !(4..=5).contains(&parts.len()) {
        return Err(format!(
            "potongan '{s}' harus LABEL:INDUK:SUMBU:OFFSET[:flip], mis. A:top:x:0"
        ));
    }
    let json = serde_json::json!({
        "label": parts[0],
        "parent": parts[1].to_ascii_lowercase(),
        "axis": parts[2].to_ascii_lowercase(),
        "offset": parts[3].parse::<f32>().map_err(|_| format!("offset '{}' bukan angka", parts[3]))?,
        "flip": match parts.get(4).map(|f| f.to_ascii_lowercase()) {
            None => false,
            Some(f) if f == "flip" => true,
            Some(f) => return Err(format!("akhiran '{f}' tidak dikenal (hanya 'flip')")),
        },
    });
    serde_json::from_value(json).map_err(|e| format!("potongan '{s}' tidak valid: {e}"))
}

fn parse_shaded(s: &str) -> Result<ShadedSpec, String> {
    serde_json::from_value(serde_json::Value::String(s.to_ascii_lowercase()))
        .map_err(|_| format!("render '{s}' tidak dikenal (iso|iso_back)"))
}

fn parse_sheet_scale(s: &str) -> Result<ScaleSpec, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| format!("skala '{s}' tidak valid (auto|1:2|0.5)"))
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
    /// Hasil studi simulasi (`Op::Study`), urut oplog.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sim: Vec<StudyRow>,
    artifacts: Vec<Artifact>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct StudyRow {
    id: String,
    /// `static` | `frequency` | `buckling` | `thermal` | `thermal_stress`.
    kind: &'static str,
    /// `ok` | `failed`.
    status: &'static str,
    /// Frekuensi natural pertama (studi frekuensi), Hz.
    #[serde(skip_serializing_if = "Option::is_none")]
    first_frequency_hz: Option<f64>,
    /// Faktor tekuk kritis (studi buckling).
    #[serde(skip_serializing_if = "Option::is_none")]
    buckling_factor: Option<f64>,
    /// Suhu maksimum (studi termal), °C.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_temperature_c: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_von_mises_mpa: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_displacement_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safety_factor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
    /// Berkas di bawah `sim/`.
    files: Vec<String>,
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

/// Massa dari densitas body: material mekanik bila ada, selain itu preset.
fn mass_g(volume_mm3: f64, density_g_cm3: Option<f64>) -> Option<f64> {
    density_g_cm3.map(|rho| volume_mm3 / 1000.0 * rho)
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
                mass_g: mass_g(volume, s.model().doc.density_of(b)).map(round2),
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
        // Material mekanik (bila dipilih) lebih bermakna di BOM daripada preset visual.
        let material = first
            .mechanical
            .as_ref()
            .map(|m| m.label())
            .unwrap_or_else(|| material_label(first.material.preset));
        let mass = mass_g(volume, s.model().doc.density_of(first))
            .map(|m| format!("{m:.2}"))
            .unwrap_or_default();
        // Sebutan standar (mis. "ISO 4762 - M6 x 20") untuk part toolbox.
        let standard = s
            .meta()
            .standard_parts
            .get(&first.name)
            .cloned()
            .unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{:.2},{},{},{}\n",
            i + 1,
            csv(&first.name),
            members.len(),
            csv(&material),
            volume,
            mass,
            csv(file),
            csv(&standard)
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
    if !r.sim.is_empty() {
        md.push_str("\n## Simulasi — estimasi teknik (sekitar ±10 % pada mesh hex bawaan)\n\n| Studi | Jenis | Status | von Mises maks (MPa) | Deformasi maks (mm) | Faktor keamanan | Frekuensi 1 (Hz) | Faktor tekuk | Suhu maks (°C) |\n|---|---|---|---:|---:|---:|---:|---:|---:|\n");
        let cell = |v: Option<f64>, digits: usize| {
            v.map(|x| format!("{x:.digits$}")).unwrap_or("—".into())
        };
        for st in r.sim.iter().take(MD_MAX_ROWS) {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                st.id,
                st.kind,
                st.status,
                cell(st.max_von_mises_mpa, 3),
                cell(st.max_displacement_mm, 5),
                cell(st.safety_factor, 2),
                cell(st.first_frequency_hz, 2),
                cell(st.buckling_factor, 3),
                cell(st.max_temperature_c, 2)
            ));
        }
        for st in r.sim.iter().take(MD_MAX_ROWS) {
            if let Some(png) = st.files.iter().find(|f| f.ends_with(".png")) {
                md.push_str(&format!("\n![{}](sim/{png})\n", st.id));
            }
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
    if !file.drawings.is_empty() {
        s.set_drawings(file.drawings);
    }
    let report = s.run(file.ops, false);
    if let Some(e) = report.error {
        return Ok((Err(e), "ops"));
    }
    match s.apply_configuration_specs(&file.configurations, file.active_configuration.as_deref()) {
        Ok(Some(r)) if r.error.is_some() => Ok((Err(r.error.unwrap_or_else(|| {
            ducad_engine::OpError::invalid("konfigurasi gagal diterapkan")
        })), "ops")),
        Ok(_) => Ok((Ok(s), "ops")),
        Err(e) => Ok((Err(e), "ops")),
    }
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

/// Baris matriks `--all-configs`.
#[derive(Serialize)]
struct ConfigRow {
    name: String,
    /// `ok` | `checks_failed` | `failed`.
    status: String,
    exit: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    checks: Option<serde_json::Value>,
}

/// `--all-configs`: satu build per konfigurasi (termasuk "Default") ke
/// `out/<nama>/`, lalu `out/report.json` + `out/report.md` berisi matriks
/// konfigurasi × check. Kode keluar = yang terburuk.
fn exec_all_configs(a: Args) -> CliResult {
    let (loaded, _) = load(&a.input)?;
    let names: Vec<String> = match loaded {
        Ok(s) => s.configurations().into_iter().map(|c| c.name).collect(),
        // Biarkan build tunggal menulis laporan gagalnya.
        Err(_) => vec![ducad_core::DEFAULT_CONFIGURATION.to_string()],
    };
    std::fs::create_dir_all(&a.out).map_err(|e| io_err(&a.out, e))?;
    let mut rows = Vec::new();
    let mut worst = Exit::Ok;
    for name in names {
        let out = a.out.join(&name);
        let single = Args {
            input: a.input.clone(),
            out: out.clone(),
            formats: a.formats.clone(),
            paper: a.paper,
            drawing: a.drawing.clone(),
            sections: a.sections.clone(),
            shaded: a.shaded.clone(),
            scale: a.scale,
            title: a.title.clone(),
            part_number: a.part_number.clone(),
            author: a.author.clone(),
            revision: a.revision.clone(),
            date: a.date.clone(),
            no_checks: a.no_checks,
            config: Some(name.clone()),
            all_configs: false,
        };
        let exit = exec(single)?;
        if exit as u8 > worst as u8 {
            worst = exit;
        }
        let report: serde_json::Value = std::fs::read_to_string(out.join("report.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        rows.push(ConfigRow {
            name,
            status: report["status"].as_str().unwrap_or("failed").to_string(),
            exit: exit as u8,
            checks: report.get("checks").cloned(),
        });
    }
    write_json(&a.out.join("report.json"), &serde_json::json!({ "configurations": rows }))?;

    // Matriks konfigurasi × check (id check sebagai kolom, urut kemunculan).
    let mut ids: Vec<String> = Vec::new();
    let cell = |row: &ConfigRow, id: &str| -> &'static str {
        let results = row.checks.as_ref().and_then(|c| c["results"].as_array());
        let found = results.and_then(|rs| {
            rs.iter().find(|r| {
                r["id"].as_str().or(r["kind"].as_str()) == Some(id)
            })
        });
        match found.and_then(|r| r["status"].as_str()) {
            Some("pass") => "✅",
            Some("fail") => "❌",
            Some(_) => "⚠️",
            None => "—",
        }
    };
    for row in &rows {
        if let Some(results) = row.checks.as_ref().and_then(|c| c["results"].as_array()) {
            for r in results {
                if let Some(id) = r["id"].as_str().or(r["kind"].as_str()) {
                    if !ids.iter().any(|x| x == id) {
                        ids.push(id.to_string());
                    }
                }
            }
        }
    }
    let title = a.title.clone().unwrap_or_else(|| stem_of(&a.input));
    let mut md = format!("# DUCAD build — {title} (semua konfigurasi)\n\n| Konfigurasi | Status |");
    for id in ids.iter().take(MD_MAX_ROWS) {
        md.push_str(&format!(" {id} |"));
    }
    md.push_str("\n|---|---|");
    md.push_str(&"---|".repeat(ids.len().min(MD_MAX_ROWS)));
    md.push('\n');
    for row in &rows {
        md.push_str(&format!("| [{0}]({0}/report.md) | {1} |", row.name, row.status));
        for id in ids.iter().take(MD_MAX_ROWS) {
            md.push_str(&format!(" {} |", cell(row, id)));
        }
        md.push('\n');
    }
    let p = a.out.join("report.md");
    std::fs::write(&p, md).map_err(|e| io_err(&p, e))?;
    Ok(worst)
}

pub fn exec(a: Args) -> CliResult {
    if a.all_configs {
        return exec_all_configs(a);
    }
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
        sim: Vec::new(),
        artifacts: Vec::new(),
        warnings: Vec::new(),
    };

    let (loaded, mode) = load(&a.input)?;
    report.load = mode;
    // Varian yang diminta diaktifkan sebelum apa pun diukur/diekspor.
    let loaded = match (loaded, &a.config) {
        (Ok(mut s), Some(name)) => match s.activate_configuration(name) {
            Ok(r) => match r.error {
                Some(e) => Err(e),
                None => Ok(s),
            },
            Err(e) if e.code == OpErrorCode::UnknownRef => {
                return Err(CliError::usage(e.message));
            }
            Err(e) => Err(e),
        },
        (other, _) => other,
    };
    let mut s = match loaded {
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

    // Studi dijalankan sebelum checks: `max_stress` dkk. butuh hasilnya.
    let mut sim_files: Vec<PathBuf> = Vec::new();
    let studies = s.run_all_studies(&ducad_sim::CancelToken::new());
    if !studies.is_empty() {
        let dir = a.out.join("sim");
        std::fs::create_dir_all(&dir).map_err(|e| io_err(&dir, e))?;
        for (id, result) in studies {
            let mut row = StudyRow {
                id: id.clone(),
                kind: ducad_engine::sim::def_of(s.meta(), &id)
                    .map(|d| d.kind.name())
                    .unwrap_or("static"),
                status: "ok",
                first_frequency_hz: None,
                buckling_factor: None,
                max_temperature_c: None,
                max_von_mises_mpa: None,
                max_displacement_mm: None,
                safety_factor: None,
                error: None,
                files: Vec::new(),
            };
            match result {
                Ok(ducad_engine::sim::StudyOutcome::Analysis(r)) => {
                    use ducad_engine::sim::AnalysisReport;
                    match r.as_ref() {
                        AnalysisReport::Frequency(f) => {
                            row.first_frequency_hz = f.frequencies_hz.first().copied()
                        }
                        AnalysisReport::Buckling(b) => {
                            row.buckling_factor = b.load_factors.first().copied()
                        }
                        AnalysisReport::Thermal(t) => {
                            row.max_temperature_c = Some(t.max_temperature_c)
                        }
                    }
                    let json = dir.join(format!("{id}.json"));
                    write_json(&json, r.as_ref())?;
                    row.files = vec![format!("{id}.json")];
                    sim_files.push(json);
                }
                Ok(ducad_engine::sim::StudyOutcome::Stress(r)) => {
                    row.max_von_mises_mpa = Some(r.max_von_mises_mpa);
                    row.max_displacement_mm = Some(r.max_displacement_mm);
                    row.safety_factor = Some(r.safety_factor);
                    let json = dir.join(format!("{id}.json"));
                    write_json(&json, r.as_ref())?;
                    let png = dir.join(format!("{id}-stress.png"));
                    super::sim::write_heatmap(
                        &s,
                        &id,
                        &r,
                        ducad_engine::sim::Overlay::Stress,
                        &png,
                    )?;
                    row.files = vec![format!("{id}.json"), format!("{id}-stress.png")];
                    sim_files.extend([json, png]);
                }
                Err(e) => {
                    row.status = "failed";
                    report
                        .warnings
                        .push(format!("studi '{id}' gagal: {}", e.message));
                    row.error = serde_json::to_value(&e).ok();
                }
            }
            report.sim.push(row);
        }
    }

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
        // Lembar tersimpan (`design.drawings`) menjadi dasar; flag CLI menimpa.
        let stored = match &a.drawing {
            Some(name) => Some(s.design().drawing(name).cloned().ok_or_else(|| {
                CliError::usage(format!(
                    "lembar gambar '{name}' tidak ada di part (tersedia: {:?})",
                    s.design().drawings.iter().map(|d| d.name.as_str()).collect::<Vec<_>>()
                ))
            })?),
            None => s.design().drawings.first().cloned(),
        };
        let from_store = stored.is_some();
        let mut spec = stored.unwrap_or_default();
        if let Some(paper) = a.paper {
            spec.paper = paper;
        }
        if !from_store || a.title.is_some() || spec.title.title.is_empty() {
            spec.title.title = title.clone();
        }
        if a.part_number.is_some() || spec.title.part_number.is_empty() {
            spec.title.part_number = a.part_number.clone().unwrap_or_else(|| stem.clone());
        }
        if !a.author.is_empty() || !from_store {
            spec.title.author = a.author.clone();
        }
        // Tanggal selalu dari build (SOURCE_DATE_EPOCH) supaya deterministik.
        spec.title.date = date.clone();
        if spec.title.material.is_empty() {
            spec.title.material = material;
        }
        if !from_store || spec.title.revision.is_empty() {
            spec.title.revision = a.revision.clone();
        }
        if !from_store {
            spec.notes = hole_notes(s.design(), &s.design().effective_params());
        }
        if !a.sections.is_empty() {
            spec.sections = Some(a.sections.clone());
        }
        if !a.shaded.is_empty() {
            spec.shaded = a.shaded.clone();
        }
        if let Some(scale) = a.scale {
            spec.scale = scale;
        }
        let output = build_sheet(s.model(), &spec, Some(&s.design().fingerprint))?;
        report.warnings.extend(output.warnings.iter().cloned());
        let sheet = output.sheet;
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

    if has("flat") {
        // Satu DXF pola bentangan per body sheet metal (urut nama).
        for (body, state) in &s.meta().sheet_metal {
            let flat = state
                .model
                .flat_pattern()
                .map_err(|e| CliError::usage(format!("pola datar '{body}': {e}")))?;
            let p = a.out.join(format!("{stem}-{body}-flat.dxf"));
            std::fs::write(&p, ducad_io::flat_dxf::flat_pattern_dxf(&flat))
                .map_err(|e| io_err(&p, e))?;
            written.push(p);
        }
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
    for p in &sim_files {
        let bytes = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let file = p
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| format!("sim/{n}"))
            .unwrap_or_default();
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
