//! Popup Akun DUCAD: foto profil akun login menggantikan inisial bila ada,
//! semua warna isian adalah premultiplied yang sah (bukan blok aditif yang
//! menyala), dan tombol keluar menghasilkan event.

use ducad_cloud::{AuthStatus, CloudFile, DucadAccount, FileList};
use ducad_core::LicenseTier;
use ducad_ui::{AccountAvatar, AccountDrawer, AccountDrawerEvent, CloudView};
use egui::epaint::Shape;

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 900.0);

fn account() -> DucadAccount {
    DucadAccount {
        user_id: "usr_1".to_string(),
        email: "pengguna@contoh.id".to_string(),
        display_name: Some("Budi Santoso".to_string()),
        avatar_url: Some("https://lh3.googleusercontent.com/a/X=s96-c".to_string()),
        username: None,
        phone: None,
        access_token: "a".to_string(),
        refresh_token: "r".to_string(),
        token_expires_at: 0,
        license_tier: "Pro".to_string(),
    }
}

struct Frame {
    shapes: Vec<Shape>,
    texts: Vec<(String, egui::Rect)>,
    event: Option<AccountDrawerEvent>,
}

fn cloud_files() -> FileList {
    FileList {
        files: vec![CloudFile {
            id: "f-1".to_string(),
            name: "braket.ducad".to_string(),
            size: 2048,
            sha256: "ab".to_string(),
            revision: 1,
            updated_at: "2026-10-20T10:00:00Z".to_string(),
        }],
        used_bytes: 2048,
        quota_bytes: 50 * 1024 * 1024,
        max_file_bytes: 32 * 1024 * 1024,
        max_files: 20,
        tier: "Pro".to_string(),
    }
}

fn frame(
    ctx: &egui::Context,
    photo: Option<&egui::TextureHandle>,
    events: Vec<egui::Event>,
) -> Frame {
    let files = cloud_files();
    let cloud = CloudView {
        tier: LicenseTier::Pro,
        files: Some(&files),
        current_document: Some("braket.ducad"),
        ..Default::default()
    };
    frame_with(ctx, photo, events, cloud)
}

fn frame_with(
    ctx: &egui::Context,
    photo: Option<&egui::TextureHandle>,
    events: Vec<egui::Event>,
    cloud: CloudView<'_>,
) -> Frame {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        events,
        ..Default::default()
    };
    let acc = account();
    let anchor = egui::Rect::from_min_size(egui::pos2(1200.0, 10.0), egui::vec2(26.0, 26.0));
    let mut event = None;
    let mut out = ctx.run_ui(input, |ui| {
        let avatar = AccountAvatar { photo, ..Default::default() };
        event = AccountDrawer::show(ui.ctx(), anchor, Some(&acc), &AuthStatus::LoggedIn, avatar, cloud);
    });
    out.textures_delta.clear();
    let mut shapes = Vec::new();
    let mut texts = Vec::new();
    for cs in out.shapes {
        if let Shape::Text(t) = &cs.shape {
            texts.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2())));
        }
        shapes.push(cs.shape);
    }
    Frame { shapes, texts, event }
}

fn settle(ctx: &egui::Context, photo: Option<&egui::TextureHandle>) -> Frame {
    let mut last = frame(ctx, photo, Vec::new());
    for _ in 0..3 {
        last = frame(ctx, photo, Vec::new());
    }
    last
}

fn photo(ctx: &egui::Context) -> egui::TextureHandle {
    let img = egui::ColorImage::new([8, 8], vec![egui::Color32::from_rgb(200, 120, 40); 64]);
    ctx.load_texture("test-avatar", img, egui::TextureOptions::LINEAR)
}

#[test]
fn initials_shown_without_photo() {
    let ctx = egui::Context::default();
    let f = settle(&ctx, None);
    assert!(f.texts.iter().any(|(t, _)| t == "BS"), "inisial harus tampil tanpa foto");
}

#[test]
fn photo_replaces_initials() {
    let ctx = egui::Context::default();
    let tex = photo(&ctx);
    let f = settle(&ctx, Some(&tex));
    assert!(
        f.shapes.iter().any(|s| s.texture_id() == tex.id()),
        "foto profil harus tergambar"
    );
    assert!(!f.texts.iter().any(|(t, _)| t == "BS"), "inisial tidak boleh menumpuk foto");
}

/// Warna premultiplied dengan RGB > alpha dianggap aditif oleh egui — itulah
/// yang dulu membuat badge tier cyan menyala dan tombol keluar merah pekat.
#[test]
fn fills_are_valid_premultiplied() {
    let ctx = egui::Context::default();
    let f = settle(&ctx, None);
    let bad = |c: egui::Color32| c.r() > c.a() || c.g() > c.a() || c.b() > c.a();
    for shape in &f.shapes {
        match shape {
            Shape::Rect(r) => assert!(!bad(r.fill), "isian kotak aditif: {:?}", r.fill),
            Shape::Circle(c) => assert!(!bad(c.fill), "isian lingkaran aditif: {:?}", c.fill),
            _ => {}
        }
    }
}

fn click_text(ctx: &egui::Context, photo: Option<&egui::TextureHandle>, label: &str) -> Option<AccountDrawerEvent> {
    let f = settle(ctx, photo);
    let Some((_, rect)) = f.texts.iter().find(|(t, _)| t.contains(label)) else {
        panic!("teks {label:?} tidak tergambar");
    };
    let pos = rect.center();
    frame(ctx, photo, vec![egui::Event::PointerMoved(pos)]);
    let press = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(ctx, photo, vec![press(true)]);
    frame(ctx, photo, vec![press(false)]).event
}

#[test]
fn upload_offered_only_without_photo() {
    let ctx = egui::Context::default();
    assert_eq!(
        click_text(&ctx, None, "Unggah foto profil"),
        Some(AccountDrawerEvent::UploadPhoto)
    );

    let tex = photo(&ctx);
    let f = settle(&ctx, Some(&tex));
    assert!(
        !f.texts.iter().any(|(t, _)| t.contains("Unggah foto profil")),
        "akun yang sudah punya foto tidak perlu tombol unggah"
    );
}

#[test]
fn logout_click_emits_event() {
    let ctx = egui::Context::default();
    assert_eq!(click_text(&ctx, None, "Keluar dari Akun"), Some(AccountDrawerEvent::Logout));
}

/// Daftar fitur lama ("Riwayat Versi & Rollback", dst.) menjanjikan fitur
/// yang tidak ada. Yang tampil sekarang harus tier nyata + berkas cloud.
#[test]
fn shows_real_tier_and_cloud_files_instead_of_placeholder_features() {
    let ctx = egui::Context::default();
    let f = settle(&ctx, None);
    let has = |s: &str| f.texts.iter().any(|(t, _)| t.contains(s));
    assert!(has("DUCAD Pro"));
    assert!(has("braket.ducad"));
    assert!(has("Cloud Sync"));
    assert!(!has("Riwayat Versi"), "fitur palsu tidak boleh tampil lagi");
    assert!(!has("Cloud Active"));
}

#[test]
fn free_tier_marks_pro_features_as_locked() {
    let ctx = egui::Context::default();
    let cloud = CloudView { tier: LicenseTier::Free, ..Default::default() };
    let mut f = frame_with(&ctx, None, Vec::new(), cloud);
    for _ in 0..3 {
        f = frame_with(&ctx, None, Vec::new(), cloud);
    }
    assert!(f.texts.iter().any(|(t, _)| t.contains("DUCAD Free")));
    assert!(f.texts.iter().any(|(t, _)| t.contains("tersedia di DUCAD Pro")));
    assert!(
        f.texts.iter().any(|(t, _)| t.contains("Simpan dokumen dulu")),
        "tanpa dokumen tersimpan, tombol unggah menjelaskan sebabnya"
    );
}

#[test]
fn upload_current_click_emits_event() {
    let ctx = egui::Context::default();
    assert_eq!(
        click_text(&ctx, None, "Unggah braket.ducad"),
        Some(AccountDrawerEvent::CloudUploadCurrent)
    );
}
