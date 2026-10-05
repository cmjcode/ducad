//! Setiap tab panel "Fitur Industri" harus tergambar (tanpa panic, tanpa
//! teks terjemahan yang hilang) dengan data kosong maupun terisi.

use ducad_ui::{
    IndConfigRow, IndSheetRow, IndStudyRow, IndustryData, IndustryPanel, IndustryTab,
};

fn frame(ctx: &egui::Context, panel: &mut IndustryPanel, data: &IndustryData) -> Vec<String> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 900.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        panel.show(ui, data);
    });
    out.textures_delta.clear();
    out.shapes
        .into_iter()
        .filter_map(|cs| match cs.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

fn filled() -> IndustryData {
    IndustryData {
        target_body: Some("beam".into()),
        target_has_material: true,
        picked_face: Some(">Z".into()),
        picked_point: Some([1.0, 2.0, 3.0]),
        studies: vec![IndStudyRow {
            id: "modes".into(),
            kind: "frequency".into(),
            body: "beam".into(),
            lines: vec!["HASIL-UJI 410 Hz".into()],
            ..IndStudyRow::default()
        }],
        configs: vec![
            IndConfigRow {
                name: "Default".into(),
                active: true,
                ..IndConfigRow::default()
            },
            IndConfigRow {
                name: "VARIAN-UJI".into(),
                params: vec![("len".into(), 80.0)],
                suppressed: vec!["holes".into()],
                ..IndConfigRow::default()
            },
        ],
        base_params: vec![("len".into(), 50.0)],
        op_ids: vec!["base".into(), "holes".into()],
        sheets: vec![IndSheetRow {
            name: "PELAT-UJI".into(),
            thickness: 2.0,
            flanges: 2,
            unfolded: false,
        }],
        annotations: vec!["ANOTASI-UJI".into()],
        instances: vec![(1, "INSTANCE-A".into()), (2, "INSTANCE-B".into())],
        couplings: vec!["KOPLING-UJI".into()],
        explode_steps: vec!["LANGKAH-UJI".into()],
        explode_factor: 0.5,
        threads: vec!["ULIR-UJI".into()],
    }
}

#[test]
fn every_tab_renders_with_empty_and_filled_data() {
    let tabs = [
        (IndustryTab::Study, "HASIL-UJI"),
        (IndustryTab::Config, "VARIAN-UJI"),
        (IndustryTab::Sheet, "PELAT-UJI"),
        (IndustryTab::Tolerance, "ANOTASI-UJI"),
        (IndustryTab::Parts, "ULIR-UJI"),
        (IndustryTab::Assembly, "KOPLING-UJI"),
    ];
    for (tab, marker) in tabs {
        let ctx = egui::Context::default();
        let mut panel = IndustryPanel::default();
        panel.tab = tab;
        let mut empty_texts = Vec::new();
        for _ in 0..3 {
            empty_texts = frame(&ctx, &mut panel, &IndustryData::default());
        }
        assert!(!empty_texts.is_empty(), "{tab:?}: panel kosong tidak tergambar");

        let data = filled();
        let mut texts = Vec::new();
        for _ in 0..3 {
            texts = frame(&ctx, &mut panel, &data);
        }
        assert!(
            texts.iter().any(|t| t.contains(marker)),
            "{tab:?}: '{marker}' tidak tergambar; teks = {texts:?}"
        );
        // Kunci terjemahan yang hilang tampil sebagai kuncinya sendiri.
        let missing: Vec<&String> = texts.iter().filter(|t| t.starts_with("ind-")).collect();
        assert!(missing.is_empty(), "{tab:?}: terjemahan hilang {missing:?}");
    }
}
