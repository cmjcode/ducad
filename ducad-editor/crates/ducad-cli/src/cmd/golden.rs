//! `ducad-cli golden` — korpus golden untuk migrasi kernel (PLAN_MIGRASI_TRUCK F0).
//!
//! `capture` me-replay setiap contoh korpus (`*.ops.json` + `*.ducad`) lalu
//! menulis satu berkas JSON per contoh: volume, luas, bbox, jumlah face/tepi,
//! sidik jari mesh, dan vonis tiap check. `check` me-replay ulang dan
//! membandingkan dengan toleransi — inilah ukuran regresi objektif saat
//! backend kernel diganti.
//!
//! Jalankan dari direktori `ducad-editor/`: path sumber di berkas golden
//! relatif terhadap direktori kerja.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ducad_engine::check::{CheckStatus, CheckSummary};
use ducad_engine::inspect::summarize;
use ducad_engine::ops::OpFile;
use ducad_engine::{OpError, Session};
use serde::{Deserialize, Serialize};

use super::{print_json, write_json};
use crate::{CliError, CliResult, Exit};

/// Versi format berkas golden; naikkan bila field berubah makna.
const SCHEMA: u32 = 1;

/// Korpus bawaan, relatif terhadap `ducad-editor/`.
const DEFAULT_CORPUS: &[&str] = &[
    "crates/ducad-engine/tests/fixtures",
    "examples",
    "evals/fixtures",
    "../examples",
];

/// Backend kernel yang sedang dikompilasi (`truck`). Golden baseline di
/// `tests/goldens/occt` bertanda `occt` (direkam 0.5.x, referensi beku).
const BACKEND: &str = ducad_kernel::BACKEND;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand)]
enum Action {
    /// Replay korpus dan tulis satu berkas golden per contoh.
    Capture(CaptureArgs),
    /// Replay korpus dan bandingkan dengan berkas golden (kode 1 bila beda).
    Check(CheckArgs),
}

#[derive(clap::Args)]
struct CorpusArgs {
    /// Direktori korpus (dipindai rekursif untuk `*.ops.json` dan `*.ducad`),
    /// boleh diulang. Default: fixture engine, `examples`, `evals/fixtures`,
    /// `../examples`.
    #[arg(long = "corpus")]
    corpus: Vec<PathBuf>,
}

#[derive(clap::Args)]
struct CaptureArgs {
    #[command(flatten)]
    corpus: CorpusArgs,
    /// Direktori keluaran golden.
    #[arg(long, default_value = "tests/goldens/occt")]
    out: PathBuf,
    /// Tulis laporan waktu op ke berkas Markdown ini (+ `.json` di sebelahnya).
    #[arg(long)]
    perf: Option<PathBuf>,
}

#[derive(clap::Args)]
struct CheckArgs {
    #[command(flatten)]
    corpus: CorpusArgs,
    /// Direktori golden pembanding.
    #[arg(long, default_value = "tests/goldens/occt")]
    against: PathBuf,
    /// Toleransi relatif volume & luas.
    #[arg(long, default_value_t = 1e-3)]
    tol: f64,
    /// Toleransi absolut bbox (mm).
    #[arg(long, default_value_t = 0.01)]
    bbox_tol: f64,
    /// Sidik jari mesh yang berbeda dihitung gagal (default: hanya dicatat,
    /// karena tessellasi berbeda antar-backend dan antar-OS).
    #[arg(long)]
    strict_mesh: bool,
    /// Hanya periksa contoh yang SELURUH op-nya ada di daftar ini (dipisah
    /// koma, mis. `sketch,primitive,extrude`); contoh lain dilewati dan
    /// dilaporkan. Dipakai gate fase migrasi: backend baru diukur hanya
    /// pada op yang sudah diporting (PLAN_MIGRASI_TRUCK F2: `--ops
    /// sketch,primitive,extrude,revolve,boolean,transform,scale,delete,set_material`).
    #[arg(long, value_delimiter = ',')]
    ops: Vec<String>,
    /// Berkas perbedaan yang DIKETAHUI & diterima (gate F3
    /// PLAN_MIGRASI_TRUCK): baris `sumber<TAB>awalan-perbedaan<TAB>alasan`,
    /// `#` = komentar. Perbedaan yang diawali awalan itu tidak menggagalkan
    /// contoh tersebut; perbedaan lain tetap gagal. Awalan `*skip*` =
    /// contoh dilewati (dilaporkan "skip" beserta alasannya).
    #[arg(long)]
    known: Option<PathBuf>,
    /// Keluaran JSON.
    #[arg(long)]
    json: bool,
}

// ---------------------------------------------------------------------------
// Rekaman
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Golden {
    schema: u32,
    source: String,
    backend: String,
    /// Kode error replay (`null` = replay sukses).
    error: Option<GoldenError>,
    oplog_len: usize,
    bodies: Vec<GoldenBody>,
    /// Vonis tiap check (`null` = contoh tanpa check).
    checks: Option<Vec<GoldenCheck>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct GoldenError {
    code: String,
    op_index: Option<usize>,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct GoldenBody {
    name: String,
    valid: bool,
    volume: f64,
    area: f64,
    bbox: [[f64; 3]; 2],
    faces: usize,
    edges: usize,
    mesh: GoldenMesh,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct GoldenMesh {
    vertices: usize,
    triangles: usize,
    /// FNV-1a 64-bit dari posisi+normal+indeks (`BodyGeometry::mesh_fingerprint`).
    sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct GoldenCheck {
    label: String,
    status: String,
}

/// Waktu replay satu contoh (tidak disimpan di golden: bising antar-mesin).
#[derive(Debug, Clone, Serialize)]
struct Timing {
    source: String,
    total_ms: f64,
    ops: Vec<OpTiming>,
}

#[derive(Debug, Clone, Serialize)]
struct OpTiming {
    id: String,
    kind: String,
    ms: f64,
}

fn round(v: f64, decimals: i32) -> f64 {
    let f = 10f64.powi(decimals);
    let r = (v * f).round() / f;
    if r == 0.0 {
        0.0 // buang -0.0 agar berkas stabil
    } else {
        r
    }
}

fn golden_error(e: &OpError) -> GoldenError {
    let code = serde_json::to_value(e.code)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", e.code));
    GoldenError {
        code,
        op_index: e.op_index,
        message: e.message.clone(),
    }
}

fn record_bodies(s: &Session) -> Result<Vec<GoldenBody>, CliError> {
    let sum = summarize(s, None, false, 0)?;
    let model = s.model();
    let mut out = Vec::with_capacity(sum.bodies.len());
    for b in &sum.bodies {
        let geo = model
            .doc
            .bodies
            .iter()
            .find(|(_, body)| body.name == b.name)
            .and_then(|(id, _)| model.geometry.get(id));
        let mesh = match geo {
            Some(g) => GoldenMesh {
                vertices: g.mesh.positions.len(),
                triangles: g.mesh.triangle_count(),
                sha: format!("{:016x}", g.mesh_fingerprint),
            },
            None => GoldenMesh {
                vertices: 0,
                triangles: 0,
                sha: String::new(),
            },
        };
        out.push(GoldenBody {
            name: b.name.clone(),
            valid: b.valid,
            volume: round(b.volume, 4),
            area: round(b.area, 4),
            bbox: b.bbox.map(|p| p.map(|v| round(v, 4))),
            faces: b.faces,
            edges: b.edges,
            mesh,
        });
    }
    Ok(out)
}

fn record_checks(s: &Session) -> Option<Vec<GoldenCheck>> {
    if s.design().checks.is_empty() {
        return None;
    }
    let summary: CheckSummary = s.run_checks(None);
    Some(
        summary
            .results
            .iter()
            .map(|r| GoldenCheck {
                label: r
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("{}#{}", r.kind, r.index)),
                status: match r.status {
                    CheckStatus::Pass => "pass",
                    CheckStatus::Fail => "fail",
                    CheckStatus::Error => "error",
                }
                .to_string(),
            })
            .collect(),
    )
}

/// Replay OpFile op demi op (agar waktu tiap op terukur), lalu pasang
/// checks/varian seperti `ducad-cli run`.
fn replay_opfile(path: &Path, timing: &mut Timing) -> Result<(Session, Option<OpError>), CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", path.display())))?;
    let file: OpFile = serde_json::from_str(&text)
        .map_err(|e| CliError::usage(format!("{} bukan OpFile yang valid: {e}", path.display())))?;
    let mut s = Session::new();
    if let Some(report) = super::apply_params(&mut s, file.params, &[])? {
        if let Some(e) = report.error {
            return Ok((s, Some(e)));
        }
    }
    for (index, op) in file.ops.into_iter().enumerate() {
        let t = Instant::now();
        let report = s.run(vec![op], false);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        if let Some(mut e) = report.error {
            e.op_index = Some(index);
            return Ok((s, Some(e)));
        }
        if let Some(o) = report.outcomes.first() {
            timing.ops.push(OpTiming {
                id: o.op_id.clone(),
                kind: o.kind.to_string(),
                ms,
            });
        }
    }
    if let Err(e) = s.install_file_data(
        file.checks,
        &file.rule_packs,
        file.drawings,
        file.brief,
        file.package,
    ) {
        return Ok((s, Some(e)));
    }
    match s.apply_configuration_specs(&file.configurations, file.active_configuration.as_deref()) {
        Ok(Some(r)) if !r.committed => Ok((s, r.error)),
        Ok(_) => Ok((s, None)),
        Err(e) => Ok((s, Some(e))),
    }
}

fn capture_one(source: &Path) -> Result<(Golden, Timing), CliError> {
    let name = source.to_string_lossy().replace('\\', "/");
    let mut timing = Timing {
        source: name.clone(),
        total_ms: 0.0,
        ops: Vec::new(),
    };
    let t = Instant::now();
    let (session, error) = if is_ducad(source) {
        match Session::from_file(source) {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e)),
        }
    } else {
        let (s, e) = replay_opfile(source, &mut timing)?;
        (Some(s), e)
    };
    timing.total_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut golden = Golden {
        schema: SCHEMA,
        source: name,
        backend: BACKEND.to_string(),
        error: error.as_ref().map(golden_error),
        oplog_len: 0,
        bodies: Vec::new(),
        checks: None,
    };
    // Replay gagal: geometri parsial tidak bermakna, cukup catat error-nya.
    if let (Some(s), None) = (&session, &error) {
        golden.oplog_len = s.design().oplog.len();
        golden.bodies = record_bodies(s)?;
        golden.checks = record_checks(s);
    }
    Ok((golden, timing))
}

// ---------------------------------------------------------------------------
// Korpus
// ---------------------------------------------------------------------------

fn is_ducad(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == "ducad")
}

fn is_corpus_file(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    name.ends_with(".ops.json") || name.ends_with(".ducad")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), CliError> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", dir.display())))?;
    for entry in entries.flatten() {
        let p = entry.path();
        let hidden = p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        if p.is_dir() {
            // Artefak build (`dist/`) bukan sumber.
            if p.file_name().is_some_and(|n| n == "dist" || n == "out") {
                continue;
            }
            walk(&p, out)?;
        } else if is_corpus_file(&p) {
            out.push(p);
        }
    }
    Ok(())
}

fn discover(args: &CorpusArgs) -> Result<Vec<PathBuf>, CliError> {
    let explicit = !args.corpus.is_empty();
    let dirs: Vec<PathBuf> = if explicit {
        args.corpus.clone()
    } else {
        DEFAULT_CORPUS.iter().map(PathBuf::from).collect()
    };
    let mut out = Vec::new();
    for d in &dirs {
        if d.is_dir() {
            walk(d, &mut out)?;
        } else if explicit && d.is_file() && is_corpus_file(d) {
            out.push(d.clone());
        } else if explicit {
            return Err(CliError::usage(format!(
                "korpus {} tidak ditemukan",
                d.display()
            )));
        }
        // Direktori default yang tidak ada (mis. checkout tanpa `../examples`) dilewati.
    }
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err(CliError::usage(
            "korpus kosong (jalankan dari direktori ducad-editor/ atau beri --corpus)",
        ));
    }
    Ok(out)
}

/// Nama berkas golden: path sumber dengan `/` → `__`, `..` → `root`,
/// akhiran `.ops.json` dibuang.
fn golden_file_name(source: &Path) -> String {
    let s = source.to_string_lossy().replace('\\', "/");
    let s = s.strip_suffix(".ops.json").unwrap_or(&s);
    let parts: Vec<&str> = s
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .map(|p| if p == ".." { "root" } else { p })
        .collect();
    format!("{}.json", parts.join("__"))
}

fn read_golden(path: &Path) -> Result<Golden, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| CliError::usage(format!("{} bukan berkas golden: {e}", path.display())))
}

// ---------------------------------------------------------------------------
// capture
// ---------------------------------------------------------------------------

fn capture(a: CaptureArgs) -> CliResult {
    let sources = discover(&a.corpus)?;
    std::fs::create_dir_all(&a.out)
        .map_err(|e| CliError::usage(format!("gagal membuat {}: {e}", a.out.display())))?;
    let mut timings = Vec::new();
    let mut written = std::collections::BTreeSet::new();
    for src in &sources {
        let (golden, timing) = capture_one(src)?;
        let file = golden_file_name(src);
        let status = match &golden.error {
            Some(e) => format!("ERROR {}", e.code),
            None => format!("{} body", golden.bodies.len()),
        };
        eprintln!("{:>9.1} ms  {}  {}", timing.total_ms, golden.source, status);
        write_json(&a.out.join(&file), &golden)?;
        written.insert(file);
        timings.push(timing);
    }
    // Golden tanpa sumber lagi akan membuat `check` gagal; buang di sini.
    if let Ok(entries) = std::fs::read_dir(&a.out) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") && !written.contains(&name) {
                eprintln!("hapus golden yatim: {name}");
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    if let Some(perf) = &a.perf {
        write_perf(perf, &timings)?;
    }
    eprintln!("{} golden ditulis ke {}", written.len(), a.out.display());
    Ok(Exit::Ok)
}

fn write_perf(path: &Path, timings: &[Timing]) -> Result<(), CliError> {
    write_json(&path.with_extension("json"), &timings)?;
    // Agregat per jenis op: (jumlah, total, maks).
    let mut by_kind: BTreeMap<&str, (usize, f64, f64)> = BTreeMap::new();
    for t in timings {
        for o in &t.ops {
            let e = by_kind.entry(o.kind.as_str()).or_insert((0, 0.0, 0.0));
            e.0 += 1;
            e.1 += o.ms;
            e.2 = e.2.max(o.ms);
        }
    }
    let mut md = String::new();
    md.push_str("# Baseline waktu op per contoh — backend ");
    md.push_str(BACKEND);
    md.push_str("\n\nDihasilkan oleh `ducad-cli golden capture --perf` (build debug kecuali disebut lain).\n");
    md.push_str("Waktu satu op mencakup hitung kernel + tessellasi + dimensi tepi (`compute::finish`).\n");
    md.push_str("Data mentah: berkas `.json` di sebelah dokumen ini.\n\n");
    md.push_str("## Per contoh\n\n| Contoh | Total (ms) | Op | Op terlambat |\n|---|---:|---:|---|\n");
    for t in timings {
        let slowest = t
            .ops
            .iter()
            .max_by(|a, b| a.ms.total_cmp(&b.ms))
            .map(|o| format!("`{}` {} ({:.1} ms)", o.id, o.kind, o.ms))
            .unwrap_or_else(|| "— (muat `.ducad`)".into());
        md.push_str(&format!(
            "| `{}` | {:.1} | {} | {} |\n",
            t.source,
            t.total_ms,
            t.ops.len(),
            slowest
        ));
    }
    md.push_str("\n## Per jenis op\n\n| Op | Jumlah | Rata-rata (ms) | Maks (ms) | Total (ms) |\n|---|---:|---:|---:|---:|\n");
    let mut kinds: Vec<_> = by_kind.into_iter().collect();
    kinds.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
    for (kind, (n, total, max)) in kinds {
        md.push_str(&format!(
            "| `{kind}` | {n} | {:.1} | {max:.1} | {total:.1} |\n",
            total / n as f64
        ));
    }
    std::fs::write(path, md)
        .map_err(|e| CliError::usage(format!("gagal menulis {}: {e}", path.display())))
}

// ---------------------------------------------------------------------------
// check
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct CaseReport {
    source: String,
    ok: bool,
    /// Perbedaan yang menggagalkan.
    diffs: Vec<String>,
    /// Perbedaan yang hanya dicatat (mesh, contoh yang kini berhasil).
    notes: Vec<String>,
    /// Perbedaan yang cocok dengan `--known` (beserta alasannya).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    known: Vec<String>,
}

/// Awalan khusus di berkas `--known`: contoh dilewati seluruhnya (mis.
/// terlalu lambat untuk CI di backend ini), dilaporkan sebagai "skip".
const SKIP: &str = "*skip*";

/// Entri `--known`: (sumber, awalan perbedaan, alasan).
fn read_known(path: &Path) -> Result<Vec<(String, String, String)>, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", path.display())))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(src), Some(prefix), Some(reason)) => {
                out.push((src.to_string(), prefix.to_string(), reason.to_string()))
            }
            _ => {
                return Err(CliError::usage(format!(
                    "{}:{}: format baris `sumber<TAB>awalan<TAB>alasan`",
                    path.display(),
                    i + 1
                )))
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
struct CheckReport {
    backend: &'static str,
    total: usize,
    passed: usize,
    failed: usize,
    /// Contoh yang dilewati `--ops`, dengan op yang belum termasuk.
    skipped: Vec<String>,
    cases: Vec<CaseReport>,
}

/// Jenis op yang dipakai sebuah sumber korpus: `ops[].op` (`*.ops.json`)
/// atau `design.oplog[].op` (`*.ducad`; berkas lama tanpa oplog = kosong).
fn op_kinds(path: &Path) -> Result<std::collections::BTreeSet<String>, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", path.display())))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| CliError::usage(format!("{} bukan JSON: {e}", path.display())))?;
    let list = v.get("ops").or_else(|| v.pointer("/design/oplog"));
    Ok(list
        .and_then(|l| l.as_array())
        .into_iter()
        .flatten()
        .filter_map(|o| o.get("op").and_then(|k| k.as_str()).map(str::to_string))
        .collect())
}

fn rel_diff(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs());
    if scale < 1e-9 {
        0.0
    } else {
        (a - b).abs() / scale
    }
}

fn compare(want: &Golden, got: &Golden, a: &CheckArgs) -> (Vec<String>, Vec<String>) {
    let mut diffs = Vec::new();
    let mut notes = Vec::new();
    match (&want.error, &got.error) {
        (Some(w), Some(g)) => {
            if w.code != g.code {
                diffs.push(format!("kode error {} → {}", w.code, g.code));
            }
            return (diffs, notes);
        }
        (Some(w), None) => {
            notes.push(format!("dulu gagal ({}), kini berhasil", w.code));
            return (diffs, notes);
        }
        (None, Some(g)) => {
            diffs.push(format!("replay gagal: {} — {}", g.code, g.message));
            return (diffs, notes);
        }
        (None, None) => {}
    }
    if want.oplog_len != got.oplog_len {
        diffs.push(format!("oplog {} → {} op", want.oplog_len, got.oplog_len));
    }
    let names = |g: &Golden| g.bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>();
    if names(want) != names(got) {
        diffs.push(format!("body {:?} → {:?}", names(want), names(got)));
    }
    for w in &want.bodies {
        let Some(g) = got.bodies.iter().find(|b| b.name == w.name) else {
            continue;
        };
        let n = &w.name;
        if w.valid != g.valid {
            diffs.push(format!("{n}: valid {} → {}", w.valid, g.valid));
        }
        let dv = rel_diff(w.volume, g.volume);
        if dv > a.tol {
            diffs.push(format!(
                "{n}: volume {} → {} (Δ {:.2e} > {:.0e})",
                w.volume, g.volume, dv, a.tol
            ));
        }
        let da = rel_diff(w.area, g.area);
        if da > a.tol {
            diffs.push(format!(
                "{n}: luas {} → {} (Δ {:.2e} > {:.0e})",
                w.area, g.area, da, a.tol
            ));
        }
        let db = (0..2)
            .flat_map(|i| (0..3).map(move |k| (i, k)))
            .map(|(i, k)| (w.bbox[i][k] - g.bbox[i][k]).abs())
            .fold(0.0, f64::max);
        // Bbox golden baseline 0.5.x diambil dari mesh tampilan, yang di lingkaran
        // jatuh DI DALAM kotak sejati sampai satu toleransi chord
        // (`clamp(diag·3e-4, 0.005, 0.05)`); backend lain melaporkan kotak
        // geometris. Selisih itu bukan regresi.
        let chord = if want.backend == "occt" && got.backend != "occt" {
            let diag = (0..3)
                .map(|k| (w.bbox[1][k] - w.bbox[0][k]).powi(2))
                .sum::<f64>()
                .sqrt();
            (diag * 3e-4).clamp(0.005, 0.05)
        } else {
            0.0
        };
        if db > a.bbox_tol + chord {
            diffs.push(format!(
                "{n}: bbox {:?} → {:?} (Δ {db:.4} mm)",
                w.bbox, g.bbox
            ));
        }
        if w.faces != g.faces || w.edges != g.edges {
            diffs.push(format!(
                "{n}: face/tepi {}/{} → {}/{}",
                w.faces, w.edges, g.faces, g.edges
            ));
        }
        if w.mesh.sha != g.mesh.sha {
            let msg = format!(
                "{n}: mesh {} → {} ({} → {} segitiga)",
                w.mesh.sha, g.mesh.sha, w.mesh.triangles, g.mesh.triangles
            );
            if a.strict_mesh {
                diffs.push(msg);
            } else {
                notes.push(msg);
            }
        }
    }
    match (&want.checks, &got.checks) {
        (Some(w), Some(g)) => {
            if w.len() != g.len() {
                diffs.push(format!("jumlah check {} → {}", w.len(), g.len()));
            }
            for (wc, gc) in w.iter().zip(g) {
                if wc != gc {
                    diffs.push(format!(
                        "check {}: {} → {} ({})",
                        wc.label, wc.status, gc.status, gc.label
                    ));
                }
            }
        }
        (None, None) => {}
        (w, g) => diffs.push(format!(
            "checks {} → {}",
            if w.is_some() { "ada" } else { "tidak ada" },
            if g.is_some() { "ada" } else { "tidak ada" }
        )),
    }
    (diffs, notes)
}

fn check(a: CheckArgs) -> CliResult {
    let sources = discover(&a.corpus)?;
    if !a.against.is_dir() {
        return Err(CliError::usage(format!(
            "direktori golden {} tidak ada (jalankan `golden capture` dulu)",
            a.against.display()
        )));
    }
    let known_list = match &a.known {
        Some(p) => read_known(p)?,
        None => Vec::new(),
    };
    let mut cases = Vec::new();
    let mut skipped = Vec::new();
    let mut expected_files = std::collections::BTreeSet::new();
    for src in &sources {
        let file = golden_file_name(src);
        expected_files.insert(file.clone());
        let path = a.against.join(&file);
        let source = src.to_string_lossy().replace('\\', "/");
        if let Some((_, _, why)) = known_list
            .iter()
            .find(|(s, prefix, _)| *s == source && prefix == SKIP)
        {
            skipped.push(format!("{source} ({why})"));
            continue;
        }
        if !a.ops.is_empty() {
            let outside: Vec<String> = op_kinds(src)?
                .into_iter()
                .filter(|k| !a.ops.contains(k))
                .collect();
            if !outside.is_empty() {
                skipped.push(format!("{source} ({})", outside.join(", ")));
                continue;
            }
        }
        if !path.is_file() {
            cases.push(CaseReport {
                source,
                ok: false,
                diffs: vec![format!(
                    "golden {file} belum ada (jalankan `ducad-cli golden capture`)"
                )],
                notes: Vec::new(),
                known: Vec::new(),
            });
            continue;
        }
        let want = read_golden(&path)?;
        let started = std::time::Instant::now();
        let (got, _) = capture_one(src)?;
        let (diffs, notes) = compare(&want, &got, &a);
        // Progres ke stderr: korpus lambat di backend baru tetap terpantau.
        eprintln!(
            "{:>9.1} ms  {}  {}",
            started.elapsed().as_secs_f64() * 1e3,
            source,
            if diffs.is_empty() { "ok" } else { "BEDA" }
        );
        let (known, diffs): (Vec<String>, Vec<String>) = {
            let mut k = Vec::new();
            let mut d = Vec::new();
            for diff in diffs {
                match known_list
                    .iter()
                    .find(|(src, prefix, _)| *src == source && diff.starts_with(prefix.as_str()))
                {
                    Some((_, _, why)) => k.push(format!("{diff} — {why}")),
                    None => d.push(diff),
                }
            }
            (k, d)
        };
        let mut notes = notes;
        for (src, prefix, _) in known_list
            .iter()
            .filter(|(src, prefix, _)| *src == source && prefix != SKIP)
        {
            if !known.iter().any(|k| k.starts_with(prefix.as_str())) {
                notes.push(format!("known `{prefix}` tidak terjadi lagi — hapus dari {src} di daftar --known"));
            }
        }
        cases.push(CaseReport {
            source,
            ok: diffs.is_empty(),
            diffs,
            notes,
            known,
        });
    }
    // Golden yang sumbernya sudah hilang dari korpus.
    if let Ok(entries) = std::fs::read_dir(&a.against) {
        let mut orphans: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".json") && !expected_files.contains(n))
            .collect();
        orphans.sort();
        for name in orphans {
            cases.push(CaseReport {
                source: name.clone(),
                ok: false,
                diffs: vec![format!("golden {name} tidak punya sumber di korpus")],
                notes: Vec::new(),
                known: Vec::new(),
            });
        }
    }
    let failed = cases.iter().filter(|c| !c.ok).count();
    let report = CheckReport {
        backend: BACKEND,
        total: cases.len(),
        passed: cases.len() - failed,
        failed,
        skipped,
        cases,
    };
    if a.json {
        print_json(&report)?;
    } else {
        for c in &report.cases {
            let tag = match (c.ok, c.known.is_empty()) {
                (true, true) => "ok  ",
                (true, false) => "ok* ",
                _ => "BEDA",
            };
            println!("{tag} {}", c.source);
            for d in &c.diffs {
                println!("      - {d}");
            }
            for k in &c.known {
                println!("      * {k}");
            }
            for n in &c.notes {
                println!("      · {n}");
            }
        }
        for s in &report.skipped {
            println!("skip {s}");
        }
        println!(
            "golden [{}]: lulus {} · beda {} · total {} · dilewati {}",
            report.backend,
            report.passed,
            report.failed,
            report.total,
            report.skipped.len()
        );
    }
    Ok(if failed == 0 { Exit::Ok } else { Exit::OpFailed })
}

pub fn exec(a: Args) -> CliResult {
    match a.action {
        Action::Capture(c) => capture(c),
        Action::Check(c) => check(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_file_names_are_flat_and_unique() {
        assert_eq!(
            golden_file_name(Path::new("examples/vector_logo.ops.json")),
            "examples__vector_logo.json"
        );
        assert_eq!(
            golden_file_name(Path::new("../examples/industrial-demo/model.ducad")),
            "root__examples__industrial-demo__model.ducad.json"
        );
        assert_ne!(
            golden_file_name(Path::new("a/b.ops.json")),
            golden_file_name(Path::new("a/b.ducad"))
        );
    }

    #[test]
    fn rel_diff_handles_zero() {
        assert_eq!(rel_diff(0.0, 0.0), 0.0);
        assert!(rel_diff(100.0, 100.05) < 1e-3);
        assert!(rel_diff(100.0, 101.0) > 1e-3);
    }

    fn body(volume: f64, sha: &str) -> GoldenBody {
        GoldenBody {
            name: "b".into(),
            valid: true,
            volume,
            area: 10.0,
            bbox: [[0.0; 3], [1.0; 3]],
            faces: 6,
            edges: 12,
            mesh: GoldenMesh {
                vertices: 8,
                triangles: 12,
                sha: sha.into(),
            },
        }
    }

    fn golden(bodies: Vec<GoldenBody>) -> Golden {
        Golden {
            schema: SCHEMA,
            source: "x".into(),
            backend: BACKEND.into(),
            error: None,
            oplog_len: 1,
            bodies,
            checks: None,
        }
    }

    fn args(strict_mesh: bool) -> CheckArgs {
        CheckArgs {
            corpus: CorpusArgs { corpus: Vec::new() },
            against: PathBuf::new(),
            tol: 1e-3,
            bbox_tol: 0.01,
            strict_mesh,
            ops: Vec::new(),
            known: None,
            json: false,
        }
    }

    #[test]
    fn op_kinds_reads_opfiles_and_ducad_oplogs() {
        let dir = std::env::temp_dir().join(format!("ducad-golden-ops-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ops = dir.join("a.ops.json");
        std::fs::write(&ops, r#"{"ops":[{"op":"sketch"},{"op":"extrude"},{"op":"sketch"}]}"#).unwrap();
        let ducad = dir.join("b.ducad");
        std::fs::write(&ducad, r#"{"design":{"oplog":[{"op":"fillet"}]}}"#).unwrap();
        let legacy = dir.join("c.ducad");
        std::fs::write(&legacy, r#"{"bodies":[]}"#).unwrap();
        let kinds = |p: &Path| op_kinds(p).unwrap().into_iter().collect::<Vec<_>>();
        assert_eq!(kinds(&ops), ["extrude", "sketch"]);
        assert_eq!(kinds(&ducad), ["fillet"]);
        assert!(kinds(&legacy).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compare_tolerates_small_volume_and_notes_mesh() {
        let want = golden(vec![body(1000.0, "aa")]);
        let got = golden(vec![body(1000.5, "bb")]);
        let (diffs, notes) = compare(&want, &got, &args(false));
        assert!(diffs.is_empty(), "{diffs:?}");
        assert_eq!(notes.len(), 1);
        let (diffs, _) = compare(&want, &got, &args(true));
        assert_eq!(diffs.len(), 1);
    }

    #[test]
    fn compare_flags_volume_and_topology() {
        let want = golden(vec![body(1000.0, "aa")]);
        let mut b = body(1010.0, "aa");
        b.faces = 7;
        let (diffs, _) = compare(&want, &golden(vec![b]), &args(false));
        assert_eq!(diffs.len(), 2, "{diffs:?}");
    }

    #[test]
    fn compare_new_failure_is_diff_but_new_success_is_note() {
        let ok = golden(vec![body(1.0, "a")]);
        let mut err = golden(Vec::new());
        err.error = Some(GoldenError {
            code: "kernel".into(),
            op_index: Some(0),
            message: "m".into(),
        });
        assert_eq!(compare(&ok, &err, &args(false)).0.len(), 1);
        let (d, n) = compare(&err, &ok, &args(false));
        assert!(d.is_empty());
        assert_eq!(n.len(), 1);
    }
}
