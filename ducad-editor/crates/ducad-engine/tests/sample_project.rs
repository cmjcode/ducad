//! Proyek contoh Step 0 (`docs/PLAN_DESIGNER_UX.md`): bracket L yang menjadi
//! workflow sasaran designer. Tes ini menjalankan alur yang sama dengan
//! skenario uji manual di `docs/CEKLIS_UJI_GUI.md` Bagian P lewat oplog:
//! buat model → ubah dimensi awal → pulih dari operasi gagal → simpan →
//! buka ulang → ekspor STEP/STL → drawing PDF/DXF.
//!
//! Volume analitik (semua dimensi dari param):
//! - profil L: base·t + (height − t)·t, di-extrude `len`
//! - fillet cekung r di sudut dalam:          +(1 − π/4)·r²·len
//! - 4 lubang M5 clearance Ø5.5 tembus t:     −4·π·2.75²·t

use std::f64::consts::PI;
use std::path::PathBuf;

use ducad_engine::drawing_auto::{auto_sheet_model, hole_notes, TitleInfo};
use ducad_engine::export::{export, ExportFormat};
use ducad_engine::ops::{OpFile, Params};
use ducad_engine::{OpErrorCode, Session};
use ducad_io::drawing::PaperSize;

const FIXTURE: &str = include_str!("fixtures/sample_bracket.ops.json");

fn fixture() -> OpFile {
    serde_json::from_str(FIXTURE).unwrap()
}

fn build() -> Session {
    let f = fixture();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

fn analytic_volume(params: &Params) -> f64 {
    let (len, base, height, t, r) = (
        params["len"],
        params["base"],
        params["height"],
        params["t"],
        params["r"],
    );
    let profile = base * t + (height - t) * t;
    profile * len + (1.0 - PI / 4.0) * r * r * len - 4.0 * PI * 2.75 * 2.75 * t
}

fn volume(s: &Session) -> f64 {
    let sum = s.summary();
    assert_eq!(sum.bodies.len(), 1, "proyek contoh harus satu body");
    assert!(sum.bodies[0].valid, "body tidak valid");
    sum.bodies[0].volume
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    let rel = (actual - expected).abs() / expected;
    assert!(
        rel < 1e-4,
        "{what}: {actual} vs analitik {expected} (rel {rel:e})"
    );
}

fn temp(tag: &str, ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ducad-sample-{tag}-{}.{ext}", std::process::id()))
}

fn title() -> TitleInfo {
    TitleInfo {
        title: "Bracket contoh".into(),
        part_number: "UX-000".into(),
        author: "ducad".into(),
        date: "2026-10-21".into(),
        material: "AL6061".into(),
        revision: "A".into(),
    }
}

/// Tugas 1 — membuat model: sketsa terkunci penuh, satu body, volume analitik.
#[test]
fn builds_fully_constrained_bracket() {
    let f = fixture();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    let sketch = &r.outcomes[0].detail;
    assert_eq!(
        sketch["dof"], 0,
        "sketsa contoh harus fully-constrained: {sketch}"
    );
    assert_eq!(sketch["closed_regions"], 1);
    assert_eq!(s.design().oplog.len(), 5);
    assert_close(
        volume(&s),
        analytic_volume(&s.design().params),
        "volume awal",
    );
}

/// Tugas 2 — mengubah dimensi awal: fitur hilir (fillet, lubang) tetap
/// mengacu pada face/tepi yang benar karena selectornya tidak memakai
/// koordinat tetap.
#[test]
fn editing_initial_dimensions_keeps_downstream_features() {
    let edits: &[&[(&str, f64)]] = &[
        &[("t", 8.0)],
        &[("base", 55.0), ("height", 50.0)],
        &[("r", 6.0)],
        &[("len", 90.0)],
        &[("t", 4.0), ("base", 30.0), ("height", 25.0), ("r", 2.0)],
    ];
    for edit in edits {
        let mut s = build();
        let mut params = s.design().params.clone();
        for (k, v) in *edit {
            params.insert((*k).to_string(), *v);
        }
        let r = s.set_params(params.clone()).unwrap();
        assert!(r.committed, "edit {edit:?} gagal: {:?}", r.error);
        assert_close(
            volume(&s),
            analytic_volume(&params),
            &format!("edit {edit:?}"),
        );
    }
}

/// Selector berbasis koordinat tetap (`[z=5]`) tidak ikut dimensi. Setelah
/// tebal diubah, engine harus menolak dengan `selector_empty`, bukan
/// diam-diam memilih tepi lain. Ini aturan referensi yang dijaga Step 1b.
#[test]
fn coordinate_selector_fails_explicitly_after_edit() {
    let naive = FIXTURE.replace("of(+Z except >Z) and of(+X except >X)", "|Y[x=5][z=5]");
    let f: OpFile = serde_json::from_str(&naive).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params.clone()).unwrap().committed);
    assert!(
        s.run(f.ops, false).committed,
        "versi selector koordinat lulus pada t=5"
    );
    let v0 = volume(&s);

    let mut params = f.params;
    params.insert("t".into(), 8.0);
    let r = s.set_params(params).unwrap();
    assert!(!r.committed);
    let e = r.error.expect("harus ada error");
    assert_eq!(e.code, OpErrorCode::SelectorEmpty);
    assert_eq!(e.op_id.as_deref(), Some("inner_fillet"));
    assert_close(volume(&s), v0, "sesi harus utuh setelah edit gagal");
}

/// Tugas 3 — memperbaiki operasi gagal: radius terlalu besar ditolak, model
/// tetap utuh, dan saran perbaikan yang terverifikasi dapat diterapkan.
#[test]
fn failed_edit_rolls_back_and_offers_verified_fix() {
    let mut s = build();
    let v0 = volume(&s);
    let len0 = s.design().oplog.len();

    let mut params = s.design().params.clone();
    params.insert("r".into(), 40.0);
    let r = s.set_params(params).unwrap();
    assert!(!r.committed);
    let e = r.error.expect("harus ada error");
    assert_eq!(e.code, OpErrorCode::FilletRadiusTooLarge);
    assert_eq!(e.op_id.as_deref(), Some("inner_fillet"));
    assert!(
        e.hint.as_deref().is_some_and(|h| !h.is_empty()),
        "error harus punya hint"
    );
    assert_close(volume(&s), v0, "rollback");
    assert_eq!(s.design().oplog.len(), len0);
    assert_eq!(s.design().params["r"], 3.0, "param lama dipertahankan");

    // Fix terkecil lolos replay penuh. Fix yang lebih besar belum tentu:
    // lihat `verified_fixes_survive_full_replay`.
    let fix = e
        .fixes
        .iter()
        .rev()
        .find(|f| f.verified)
        .expect("ada fix terverifikasi");
    let r = s
        .replace_op("inner_fillet", fix.patched_op.clone())
        .unwrap();
    assert!(r.committed, "fix terverifikasi harus lulus: {:?}", r.error);
    assert!(
        volume(&s) > v0,
        "radius fillet lebih besar menambah material"
    );
}

/// Target Step 3: fix bertanda `verified` harus lolos replay seluruh oplog,
/// bukan hanya op yang gagal. Saat ini fix r=26.9 untuk `inner_fillet`
/// lulus sendirian tetapi membuat `base_holes` gagal (`hole_outside_face`)
/// karena face alas menyempit.
#[test]
#[ignore = "Step 3: verifikasi fix belum me-replay op hilir"]
fn verified_fixes_survive_full_replay() {
    let mut s = build();
    let mut params = s.design().params.clone();
    params.insert("r".into(), 40.0);
    let e = s
        .set_params(params)
        .unwrap()
        .error
        .expect("harus ada error");
    for fix in e.fixes.iter().filter(|f| f.verified) {
        let r = s.edit_oplog(
            None,
            vec![ducad_engine::ReplaceOp {
                id: "inner_fillet".into(),
                op: fix.patched_op.clone(),
            }],
            Vec::new(),
            true,
        );
        let r = r.unwrap();
        assert!(
            r.error.is_none(),
            "fix '{}' gagal di replay: {:?}",
            fix.label,
            r.error
        );
    }
}

/// Tugas 4 — simpan dan buka ulang: oplog, param, dan geometri sama.
#[test]
fn save_and_reopen_roundtrip() {
    let mut s = build();
    let path = temp("roundtrip", "ducad");
    s.save(&path).unwrap();
    let loaded = Session::from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.design().oplog.len(), s.design().oplog.len());
    assert_eq!(loaded.design().params, s.design().params);
    assert_close(volume(&loaded), volume(&s), "volume setelah buka ulang");
    let (a, b) = (s.summary().bodies[0].bbox, loaded.summary().bodies[0].bbox);
    for i in 0..2 {
        for j in 0..3 {
            assert!((a[i][j] - b[i][j]).abs() < 1e-6, "bbox {a:?} vs {b:?}");
        }
    }
}

/// Tugas 5 — ekspor: STEP terbuka kembali dengan volume yang sama; STL
/// tidak kosong.
#[test]
fn export_step_reopens_and_stl_is_written() {
    let s = build();
    let step = temp("export", "step");
    let stl = temp("export", "stl");
    assert!(export(&s, ExportFormat::Step, &step).unwrap() > 0);
    assert!(
        export(&s, ExportFormat::Stl, &stl).unwrap() > 84,
        "STL biner berisi segitiga"
    );

    let shape = ducad_io::step_io::import(&step).unwrap();
    let _ = std::fs::remove_file(&step);
    let _ = std::fs::remove_file(&stl);
    assert_close(shape.volume().abs(), volume(&s), "volume STEP dibuka ulang");
}

/// Tugas 6 — drawing sederhana: tampak, dimensi, catatan lubang, PDF & DXF.
#[test]
fn drawing_exports_pdf_and_dxf() {
    let s = build();
    let notes = hole_notes(s.design(), &s.design().params);
    assert!(!notes.is_empty(), "catatan lubang M5 harus muncul");
    let sheet = auto_sheet_model(s.model(), PaperSize::A3Landscape, &title(), &notes).unwrap();
    assert!(sheet.view_placements.len() >= 3, "minimal tiga tampak");
    assert!(!sheet.auto_dimensions.is_empty(), "dimensi otomatis");

    let pdf = ducad_io::pdf::generate_pdf_bytes(&sheet);
    assert!(pdf.starts_with(b"%PDF"));
    let dxf = temp("drawing", "dxf");
    ducad_io::dxf::export_drawing_sheet(&sheet, &dxf).unwrap();
    let text = std::fs::read_to_string(&dxf).unwrap();
    let _ = std::fs::remove_file(&dxf);
    assert!(
        text.contains("ENTITIES") && text.contains("LINE"),
        "DXF berisi garis"
    );
}

/// Target Step 1a: perubahan parameter adalah satu langkah undo. Saat ini
/// `Session::undo` hanya mencabut batch op terakhir; `set_params`,
/// `replace_op`, dan `remove_op` tidak tercatat di riwayat undo.
#[test]
#[ignore = "Step 1a: undo perubahan parameter belum didukung"]
fn param_edit_is_one_undo_step() {
    let mut s = build();
    let v0 = volume(&s);
    let mut params = s.design().params.clone();
    params.insert("t".into(), 8.0);
    assert!(s.set_params(params).unwrap().committed);
    assert!(s.undo().unwrap());
    assert_eq!(s.design().params["t"], 5.0);
    assert_eq!(s.design().oplog.len(), 5, "undo tidak boleh mencabut op");
    assert_close(volume(&s), v0, "undo parameter");
}

/// Baseline waktu (bukan gate): `cargo test --release -p ducad-engine
/// --test sample_project -- --ignored baseline_timings --nocapture`.
#[test]
#[ignore = "pengukuran baseline, jalankan manual"]
fn baseline_timings() {
    use std::time::Instant;
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1e3;

    let t = Instant::now();
    let mut s = build();
    let build_ms = ms(t);

    let mut params = s.design().params.clone();
    params.insert("t".into(), 8.0);
    let t = Instant::now();
    assert!(s.set_params(params).unwrap().committed);
    let regen_ms = ms(t);

    let path = temp("timing", "ducad");
    let t = Instant::now();
    s.save(&path).unwrap();
    let save_ms = ms(t);
    let t = Instant::now();
    let _ = Session::from_file(&path).unwrap();
    let open_ms = ms(t);
    let _ = std::fs::remove_file(&path);

    let step = temp("timing", "step");
    let t = Instant::now();
    export(&s, ExportFormat::Step, &step).unwrap();
    let step_ms = ms(t);
    let _ = std::fs::remove_file(&step);

    let t = Instant::now();
    let notes = hole_notes(s.design(), &s.design().params);
    let sheet = auto_sheet_model(s.model(), PaperSize::A3Landscape, &title(), &notes).unwrap();
    let _ = ducad_io::pdf::generate_pdf_bytes(&sheet);
    let drawing_ms = ms(t);

    println!(
        "build {build_ms:.1} ms | regen t=8 {regen_ms:.1} ms | save {save_ms:.1} ms | \
         open {open_ms:.1} ms | step {step_ms:.1} ms | drawing+pdf {drawing_ms:.1} ms"
    );
}
