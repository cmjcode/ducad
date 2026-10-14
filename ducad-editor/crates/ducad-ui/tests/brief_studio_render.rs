//! Layar "Proyek baru dari brief": ketiga fase tergambar, tombol terkunci
//! pada keadaan yang salah, dan klik di lembar menaruh tanda koreksi.

use ducad_i18n::t;
use ducad_io::drawing::pages::{document_pages, Block};
use ducad_io::drawing::scene::build_scene;
use ducad_io::drawing::{PaperSize, TitleBlockInfo};
use ducad_ui::{
    BriefLogUi, BriefPhase, BriefStepState, BriefStepUi, BriefStudio, BriefStudioData,
    BriefStudioEvent, BriefStudioState, SceneView,
};

const SCREEN: egui::Vec2 = egui::vec2(1440.0, 900.0);

struct Frame {
    texts: Vec<(String, egui::Rect)>,
    event: Option<BriefStudioEvent>,
}

fn frame(
    ctx: &egui::Context,
    state: &mut BriefStudioState,
    data: &BriefStudioData,
    events: Vec<egui::Event>,
) -> Frame {
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN);
    let mut event = None;
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(bounds),
            events,
            ..Default::default()
        },
        |ui| {
            egui::Area::new(egui::Id::new("brief-test"))
                .fixed_pos(bounds.min)
                .show(ui.ctx(), |ui| {
                    ui.set_width(bounds.width());
                    ui.set_height(bounds.height());
                    event = BriefStudio::show(ui, state, data);
                });
        },
    );
    out.textures_delta.clear();
    let mut texts = Vec::new();
    for cs in out.shapes {
        if let egui::epaint::Shape::Text(t) = cs.shape {
            let r = t.galley.rect.translate(t.pos.to_vec2());
            if cs.clip_rect.intersects(r) {
                texts.push((t.galley.text().to_string(), r));
            }
        }
    }
    Frame { texts, event }
}

fn settle(ctx: &egui::Context, state: &mut BriefStudioState, data: &BriefStudioData) -> Frame {
    let mut last = frame(ctx, state, data, Vec::new());
    for _ in 0..3 {
        last = frame(ctx, state, data, Vec::new());
    }
    last
}

fn click_at(
    ctx: &egui::Context,
    state: &mut BriefStudioState,
    data: &BriefStudioData,
    pos: egui::Pos2,
) -> Option<BriefStudioEvent> {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(ctx, state, data, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, state, data, vec![button(true)]);
    frame(ctx, state, data, vec![button(false)]).event
}

fn click(
    ctx: &egui::Context,
    state: &mut BriefStudioState,
    data: &BriefStudioData,
    needle: &str,
) -> Option<BriefStudioEvent> {
    let shown = settle(ctx, state, data);
    let pos = shown
        .texts
        .iter()
        .find(|(s, _)| s.contains(needle))
        .map(|(_, r)| r.center())
        .unwrap_or_else(|| panic!("teks {needle:?} tidak tergambar"));
    click_at(ctx, state, data, pos)
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    ducad_ui::theme::apply(&ctx, ducad_ui::theme::ThemeMode::Dark);
    ctx
}

fn result_data() -> BriefStudioData {
    let pages = document_pages(
        PaperSize::A3Landscape,
        &TitleBlockInfo::default(),
        &[
            Block::Heading("SENSOR BRACKET".into()),
            Block::Text("Cover".into()),
        ],
    );
    let mut scene = build_scene(&pages[0].sheet);
    scene.groups.extend(pages[0].groups.iter().cloned());
    BriefStudioData {
        phase: BriefPhase::Result,
        pages: vec!["Cover".into(), "Part - bracket".into(), "Compliance".into()],
        scene: Some(scene),
        revision: "A".into(),
        title: "Sensor bracket".into(),
        checks: (10, 0, 0),
        requirements: (5, 0, 0),
        ..BriefStudioData::default()
    }
}

#[test]
fn compose_phase_asks_for_a_brief_and_gates_the_button() {
    let ctx = ctx();
    let mut state = BriefStudioState {
        open: true,
        ..Default::default()
    };
    let data = BriefStudioData::default();
    let shown = settle(&ctx, &mut state, &data);
    let has = |n: &str| shown.texts.iter().any(|(s, _)| s.contains(n));
    for needle in [
        t!("brief-title"),
        t!("brief-compose-heading"),
        t!("brief-attach"),
        t!("brief-generate"),
        t!("brief-examples"),
    ] {
        assert!(has(&needle), "{needle:?} tidak tergambar");
    }
    // Brief kosong: tombol utama terkunci.
    assert_eq!(click(&ctx, &mut state, &data, &t!("brief-generate")), None);
    state.brief = "bracket L 60 x 40".into();
    assert_eq!(
        click(&ctx, &mut state, &data, &t!("brief-generate")),
        Some(BriefStudioEvent::Generate)
    );
    // Agent belum siap: terkunci lagi dan ada jalan ke pengaturan.
    let blocked = BriefStudioData {
        agent_blocked: Some("AI belum diizinkan".into()),
        ..BriefStudioData::default()
    };
    assert_eq!(
        click(&ctx, &mut state, &blocked, &t!("brief-generate")),
        None
    );
    assert_eq!(
        click(&ctx, &mut state, &blocked, &t!("brief-open-chat-settings")),
        Some(BriefStudioEvent::OpenChatSettings)
    );
    assert_eq!(
        click(&ctx, &mut state, &data, &t!("brief-attach")),
        Some(BriefStudioEvent::AttachImage)
    );
    assert_eq!(state.process_key(), "cnc");
}

#[test]
fn running_phase_shows_steps_log_and_stop() {
    let ctx = ctx();
    let mut state = BriefStudioState {
        open: true,
        ..Default::default()
    };
    let data = BriefStudioData {
        phase: BriefPhase::Running,
        busy: true,
        steps: vec![
            BriefStepUi {
                label: "Understand the brief".into(),
                state: BriefStepState::Done,
            },
            BriefStepUi {
                label: "Model the part".into(),
                state: BriefStepState::Active,
            },
            BriefStepUi {
                label: "Prepare the document set".into(),
                state: BriefStepState::Pending,
            },
        ],
        log: vec![
            BriefLogUi {
                ok: None,
                text: "Membuat alas.".into(),
            },
            BriefLogUi {
                ok: Some(true),
                text: "run_ops: committed".into(),
            },
        ],
        ..BriefStudioData::default()
    };
    let shown = settle(&ctx, &mut state, &data);
    let has = |n: &str| shown.texts.iter().any(|(s, _)| s.contains(n));
    for needle in [
        "Understand the brief",
        "Model the part",
        "Membuat alas.",
        "run_ops: committed",
    ] {
        assert!(has(needle), "{needle:?} tidak tergambar");
    }
    assert!(has(&t!("brief-running-heading")));
    assert_eq!(
        click(&ctx, &mut state, &data, &t!("brief-stop")),
        Some(BriefStudioEvent::Stop)
    );
}

#[test]
fn result_phase_pages_marks_revision_and_approval() {
    let ctx = ctx();
    let mut state = BriefStudioState {
        open: true,
        ..Default::default()
    };
    let data = result_data();
    let shown = settle(&ctx, &mut state, &data);
    let has = |n: &str| shown.texts.iter().any(|(s, _)| s.contains(n));
    for needle in [
        "Sensor bracket",
        "1. Cover",
        "2. Part - bracket",
        "3. Compliance",
        "SENSOR BRACKET",
    ] {
        assert!(has(needle), "{needle:?} tidak tergambar");
    }
    assert!(has(&t!("brief-revise-heading")) && has(&t!("brief-approve-heading")));

    // Pindah lembar lewat daftar.
    assert_eq!(click(&ctx, &mut state, &data, "2. Part - bracket"), None);
    assert_eq!(state.page, 1);

    // Klik di tengah lembar menaruh tanda; tanpa teks, revisi belum bisa dikirim.
    let center = egui::pos2(SCREEN.x * 0.5 - 40.0, SCREEN.y * 0.55);
    assert_eq!(click_at(&ctx, &mut state, &data, center), None);
    assert_eq!(state.marks.len(), 1, "klik di lembar menaruh satu tanda");
    assert_eq!(state.marks[0].page, 1);
    let mm = state.marks[0].position;
    assert!(
        mm[0] > 0.0 && mm[0] < 420.0 && mm[1] > 0.0 && mm[1] < 297.0,
        "{mm:?}"
    );
    assert_eq!(
        click(&ctx, &mut state, &data, &t!("brief-revise-send")),
        None
    );
    state.marks[0].text = "tebalkan jadi 8".into();
    assert_eq!(
        click(&ctx, &mut state, &data, &t!("brief-revise-send")),
        Some(BriefStudioEvent::SendRevision)
    );

    // Persetujuan: butuh nama dan tanpa penghalang.
    assert_eq!(click(&ctx, &mut state, &data, &t!("brief-approve")), None);
    state.approver = "budi".into();
    let mut blocked = result_data();
    blocked.blockers = vec!["A blocking check fails".into()];
    assert_eq!(
        click(&ctx, &mut state, &blocked, &t!("brief-approve")),
        None
    );
    assert_eq!(
        click(&ctx, &mut state, &data, &t!("brief-approve")),
        Some(BriefStudioEvent::Approve)
    );
    // Selama agent merevisi: tidak bisa mengirim revisi kedua atau menandai.
    let mut busy = result_data();
    busy.busy = true;
    let before = state.marks.len();
    click_at(&ctx, &mut state, &busy, center);
    assert_eq!(state.marks.len(), before);
    assert_eq!(
        click(&ctx, &mut state, &busy, &t!("brief-revise-send")),
        None
    );
    assert_eq!(
        click(&ctx, &mut state, &data, &t!("brief-export-pdf")),
        Some(BriefStudioEvent::ExportPdf)
    );
}

#[test]
fn scene_view_maps_sheet_mm_to_screen_and_back() {
    let data = result_data();
    let scene = data.scene.unwrap();
    let rect = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(800.0, 500.0));
    let view = SceneView::fit(&scene, rect);
    // Kertas A3 lanskap dipaskan tanpa mengubah rasio dan tetap di dalam kotak.
    assert!((view.paper.width() / view.paper.height() - 420.0 / 297.0).abs() < 1e-3);
    assert!(rect.contains_rect(view.paper));
    // Pojok kiri-bawah kertas = (0, 0) mm; Y kertas ke atas.
    let origin = view.to_screen([0.0, 0.0]);
    assert!(
        (origin.x - view.paper.min.x).abs() < 1e-3 && (origin.y - view.paper.max.y).abs() < 1e-3
    );
    let back = view.to_mm(view.to_screen([123.0, 45.0]));
    assert!((back[0] - 123.0).abs() < 1e-2 && (back[1] - 45.0).abs() < 1e-2);
}
