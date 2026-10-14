//! Panel Popup & Pengaturan Akun DUCAD / Cloud Sync untuk DUCAD.

use ducad_cloud::{AuthStatus, DucadAccount, OAuthProvider};
use egui::{
    Align, Align2, Area, Color32, CornerRadius, Frame, Layout, Margin, Order, Pos2, Rect,
    RichText, Sense, Stroke, TextureHandle, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_ADD_A_PHOTO, ICON_APPLE, ICON_CHECK, ICON_CLOSE, ICON_CLOUD, ICON_GITHUB, ICON_GOOGLE, ICON_LOGOUT,
    ICON_SYNC,
};

use crate::theme::{
    ACCENT_BLUE, ACCENT_GREEN, ACCENT_RED, AVATAR_FILL, BG_CARD_DARK, BG_POPUP_DARK,
    BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY,
};

/// Menggambar avatar bulat di `rect`: foto profil akun login bila sudah
/// termuat, selain itu inisial di atas lingkaran biru tua (Apple tidak
/// memberi foto; unduhan juga bisa gagal/offline). Hanya melukis — tidak
/// mengalokasikan widget, jadi aman dipakai di atas tombol yang sudah ada.
pub fn paint_avatar(
    ui: &Ui,
    rect: Rect,
    photo: Option<&TextureHandle>,
    initials: &str,
    ring: Stroke,
) {
    let radius = rect.width().min(rect.height()) / 2.0;
    let center = rect.center();
    let painter = ui.painter();
    match photo {
        Some(texture) => {
            let circle = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
            egui::Image::new(texture)
                .corner_radius(CornerRadius::same(radius.round() as u8))
                .paint_at(ui, circle);
        }
        None => {
            painter.circle_filled(center, radius, AVATAR_FILL);
            painter.text(
                center,
                Align2::CENTER_CENTER,
                initials,
                egui::FontId::proportional((radius * 0.8).max(9.0)),
                Color32::WHITE,
            );
        }
    }
    painter.circle_stroke(center, radius, ring);
}

/// Keadaan foto profil yang ditampilkan popup Akun.
#[derive(Clone, Copy, Default)]
pub struct AccountAvatar<'a> {
    /// Foto yang sudah termuat; `None` = tampilkan inisial.
    pub photo: Option<&'a TextureHandle>,
    /// Foto dari URL akun sedang diunduh — tombol unggah belum ditawarkan.
    pub loading: bool,
    /// Unggahan foto baru sedang berjalan.
    pub uploading: bool,
    /// Pesan galat unggahan terakhir, untuk pengguna.
    pub error: Option<&'a str>,
}

/// Event yang dihasilkan oleh `AccountDrawer`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountDrawerEvent {
    Login(OAuthProvider),
    /// Pengguna tanpa foto profil memilih untuk mengunggah foto sendiri.
    UploadPhoto,
    Logout,
    CancelLogin,
    Close,
}

pub struct AccountDrawer;

impl AccountDrawer {
    /// Menampilkan popup Akun di bawah tombol profil header
    pub fn show(
        ctx: &egui::Context,
        anchor_rect: Rect,
        account: Option<&DucadAccount>,
        auth_status: &AuthStatus,
        avatar: AccountAvatar<'_>,
    ) -> Option<AccountDrawerEvent> {
        let mut event = None;

        let popup_pos = Pos2::new(
            (anchor_rect.right() - 320.0).max(12.0),
            anchor_rect.bottom() + 6.0,
        );

        let area_res = Area::new(egui::Id::new("ducad_account_popup"))
            .order(Order::Foreground)
            .fixed_pos(popup_pos)
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .fill(BG_POPUP_DARK)
                    .corner_radius(CornerRadius::same(12))
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(180),
                    })
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_width(290.0);

                        // Header popup
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{} Akun DUCAD", ICON_CLOUD.codepoint))
                                    .size(15.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let close_btn = ui.add(
                                    egui::Button::new(
                                        RichText::new(ICON_CLOSE.codepoint)
                                            .size(14.0)
                                            .color(TEXT_SECONDARY),
                                    )
                                    .frame(false),
                                );
                                if close_btn.clicked() {
                                    event = Some(AccountDrawerEvent::Close);
                                }
                            });
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        match auth_status {
                            AuthStatus::Authenticating { provider } => {
                                Self::render_authenticating(ui, *provider, &mut event);
                            }
                            _ => {
                                if let Some(acc) = account {
                                    Self::render_logged_in(ui, acc, avatar, &mut event);
                                } else {
                                    Self::render_logged_out(ui, &mut event);
                                }
                            }
                        }
                    });
            });

        // Klik di luar area popup untuk menutup
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if !area_res.response.rect.contains(pos) && !anchor_rect.contains(pos) {
                    return Some(AccountDrawerEvent::Close);
                }
            }
        }

        event
    }

    fn render_logged_in(
        ui: &mut Ui,
        account: &DucadAccount,
        avatar: AccountAvatar<'_>,
        event: &mut Option<AccountDrawerEvent>,
    ) {
        // Unggah foto hanya ditawarkan bila akun memang tidak punya foto
        // (mis. Apple) atau foto provider gagal dimuat — bukan selagi diunduh.
        let can_upload = avatar.photo.is_none() && !avatar.loading && !avatar.uploading;

        // Kartu profil: foto akun login (atau inisial) + nama + email
        ui.horizontal(|ui| {
            let sense = if can_upload { Sense::click() } else { Sense::hover() };
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(40.0), sense);
            paint_avatar(
                ui,
                rect,
                avatar.photo,
                &account.initials(),
                Stroke::new(1.5, ACCENT_BLUE),
            );
            if can_upload {
                // Lencana kamera kecil di pojok: avatar bisa diklik untuk unggah.
                let badge = rect.right_bottom() - Vec2::splat(6.0);
                ui.painter().circle_filled(badge, 8.0, ACCENT_BLUE);
                ui.painter().circle_stroke(badge, 8.0, Stroke::new(1.5, BG_POPUP_DARK));
                ui.painter().text(
                    badge,
                    Align2::CENTER_CENTER,
                    ICON_ADD_A_PHOTO.codepoint,
                    egui::FontId::proportional(10.0),
                    Color32::WHITE,
                );
                if resp.on_hover_text("Unggah foto profil").clicked() {
                    *event = Some(AccountDrawerEvent::UploadPhoto);
                }
            }

            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(account.display_title())
                        .size(14.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(&account.email)
                        .size(11.0)
                        .color(TEXT_SECONDARY),
                );
            });
        });

        if can_upload {
            ui.add_space(8.0);
            let upload_btn = ui.add_sized(
                [ui.available_width(), 28.0],
                egui::Button::new(
                    RichText::new(format!("{}  Unggah foto profil", ICON_ADD_A_PHOTO.codepoint))
                        .size(12.0)
                        .color(TEXT_PRIMARY),
                )
                .fill(BG_CARD_DARK)
                .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                .corner_radius(CornerRadius::same(6)),
            );
            if upload_btn.clicked() {
                *event = Some(AccountDrawerEvent::UploadPhoto);
            }
        } else if avatar.uploading {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{}  Mengunggah foto profil…", ICON_SYNC.codepoint))
                    .size(11.5)
                    .color(TEXT_SECONDARY),
            );
        }
        if let Some(err) = avatar.error {
            ui.add_space(4.0);
            ui.label(RichText::new(err).size(11.0).color(ACCENT_RED));
        }

        ui.add_space(10.0);

        // Badge status & lisensi. Tint dibuat dengan `gamma_multiply` dari
        // warna opak (hasilnya premultiplied yang benar). Jangan pakai
        // `from_rgba_premultiplied` dengan RGB > alpha: egui menganggapnya
        // aditif sehingga badge tampil sebagai blok cyan menyala.
        Frame::new()
            .fill(ACCENT_BLUE.gamma_multiply(0.14))
            .corner_radius(CornerRadius::same(6))
            .stroke(Stroke::new(1.0, ACCENT_BLUE.gamma_multiply(0.45)))
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 3.5, ACCENT_GREEN);
                    ui.label(
                        RichText::new(format!("Ducad {} Tier — Cloud Active", account.license_tier))
                            .size(11.5)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                });
            });

        ui.add_space(10.0);

        // Daftar fitur
        ui.label(
            RichText::new("Fitur Cloud Terhubung:")
                .size(11.0)
                .strong()
                .color(TEXT_SECONDARY),
        );
        ui.add_space(2.0);
        for feature in [
            "Sinkronisasi Proyek CAD Cloud",
            "Riwayat Versi & Rollback",
            "Standard Parts & Fastener Catalog",
        ] {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(ICON_CHECK.codepoint).size(12.0).color(ACCENT_GREEN));
                ui.label(RichText::new(feature).size(11.0).color(TEXT_PRIMARY));
            });
        }

        ui.add_space(12.0);

        // Tombol keluar: gaya destruktif sekunder — isi merah tipis, teks
        // merah terang supaya terbaca di atas latar popup gelap.
        let logout_btn = ui.add_sized(
            [ui.available_width(), 30.0],
            egui::Button::new(
                RichText::new(format!("{} Keluar dari Akun", ICON_LOGOUT.codepoint))
                    .size(12.5)
                    .strong()
                    .color(ACCENT_RED),
            )
            .fill(ACCENT_RED.gamma_multiply(0.12))
            .stroke(Stroke::new(1.0, ACCENT_RED.gamma_multiply(0.5)))
            .corner_radius(CornerRadius::same(6)),
        );

        if logout_btn.clicked() {
            *event = Some(AccountDrawerEvent::Logout);
        }
    }

    fn render_logged_out(ui: &mut Ui, event: &mut Option<AccountDrawerEvent>) {
        ui.label(
            RichText::new("Masuk dengan Akun DUCAD untuk menikmati sinkronisasi proyek CAD cloud dan kolaborasi multi-user.")
                .size(12.0)
                .color(TEXT_SECONDARY),
        );

        ui.add_space(14.0);

        // Urutan tombol datang dari `OAuthProvider::all()`: Apple lebih dulu
        // karena Human Interface Guidelines menuntut Sign in with Apple tidak
        // kalah menonjol dari opsi login lain. Semua tombol setinggi 34 px
        // dan selebar panel, jadi "sama menonjol" terpenuhi secara harfiah.
        for (index, provider) in OAuthProvider::all().into_iter().enumerate() {
            if index > 0 {
                ui.add_space(6.0);
            }
            if Self::render_login_button(ui, provider).clicked() {
                *event = Some(AccountDrawerEvent::Login(provider));
            }
        }
    }

    /// Satu tombol login. Warna per provider mengikuti pedoman merek
    /// masing-masing: hitam pekat + logo putih untuk Apple (salah satu dari
    /// tiga gaya yang diizinkan Apple), biru Google, dan permukaan gelap
    /// DUCAD untuk GitHub.
    fn render_login_button(ui: &mut Ui, provider: OAuthProvider) -> egui::Response {
        let (icon, label, fill, stroke) = match provider {
            OAuthProvider::Apple => (
                ICON_APPLE.codepoint,
                "Masuk dengan Apple",
                Color32::BLACK,
                Some(Stroke::new(1.0, BORDER_SUBTLE)),
            ),
            OAuthProvider::Google => (
                ICON_GOOGLE.codepoint,
                "Masuk dengan Google",
                Color32::from_rgb(37, 99, 235),
                None,
            ),
            OAuthProvider::GitHub => (
                ICON_GITHUB.codepoint,
                "Masuk dengan GitHub",
                BG_CARD_DARK,
                Some(Stroke::new(1.0, BORDER_SUBTLE)),
            ),
        };

        let mut button = egui::Button::new(
            RichText::new(format!("{}  {}", icon, label))
                .size(13.0)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(fill)
        .corner_radius(CornerRadius::same(6));

        if let Some(stroke) = stroke {
            button = button.stroke(stroke);
        }

        ui.add_sized([ui.available_width(), 34.0], button)
    }

    fn render_authenticating(
        ui: &mut Ui,
        provider: OAuthProvider,
        event: &mut Option<AccountDrawerEvent>,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(ICON_SYNC.codepoint)
                    .size(24.0)
                    .color(ACCENT_BLUE),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Menunggu login via {}...", provider.label()))
                    .size(13.0)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Silakan selesaikan otentikasi di jendela browser yang terbuka.")
                    .size(11.0)
                    .color(TEXT_SECONDARY),
            );

            ui.add_space(14.0);

            let cancel_btn = ui.add_sized(
                [120.0, 28.0],
                egui::Button::new(RichText::new("Batal").size(12.0).color(TEXT_SECONDARY))
                    .corner_radius(CornerRadius::same(6)),
            );

            if cancel_btn.clicked() {
                *event = Some(AccountDrawerEvent::CancelLogin);
            }
        });
    }
}
