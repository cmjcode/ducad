//! Panel Popup & Pengaturan Akun CMJCode / Cloud Sync untuk Ducad.

use ducad_cloud::{AuthStatus, DucadAccount, OAuthProvider};
use egui::{
    Align, Align2, Area, Color32, CornerRadius, Frame, Layout, Margin, Order, Pos2, Rect,
    RichText, Sense, Stroke, Ui, Vec2,
};
use egui_icons::icons::{ICON_CLOSE, ICON_CLOUD, ICON_LOGOUT, ICON_SYNC};

use crate::theme::{
    ACCENT_BLUE, BG_CARD_DARK, BG_PANEL_DARK, BORDER_SUBTLE,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

/// Event yang dihasilkan oleh `AccountDrawer`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountDrawerEvent {
    Login(OAuthProvider),
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
        _server_url: &str,
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
                    .fill(BG_PANEL_DARK)
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
                                RichText::new(format!("{} Akun CMJCode", ICON_CLOUD.codepoint))
                                    .size(15.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let close_btn = ui.add(
                                    egui::Button::new(
                                        RichText::new(ICON_CLOSE.codepoint)
                                            .size(14.0)
                                            .color(TEXT_MUTED),
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
                                    Self::render_logged_in(ui, acc, &mut event);
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
        event: &mut Option<AccountDrawerEvent>,
    ) {
        // User Profile Card
        ui.horizontal(|ui| {
            // Avatar / Initial Circle
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 20.0, Color32::from_rgb(30, 58, 138));
            ui.painter().circle_stroke(
                rect.center(),
                20.0,
                Stroke::new(1.5, Color32::from_rgb(56, 189, 248)),
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                account.initials(),
                egui::FontId::proportional(15.0),
                Color32::WHITE,
            );

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
                        .color(TEXT_MUTED),
                );
            });
        });

        ui.add_space(10.0);

        // Status & License Badge
        Frame::new()
            .fill(Color32::from_rgba_premultiplied(56, 189, 248, 20))
            .corner_radius(CornerRadius::same(6))
            .stroke(Stroke::new(1.0, Color32::from_rgba_premultiplied(56, 189, 248, 60)))
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("●")
                            .size(10.0)
                            .color(Color32::from_rgb(74, 222, 128)),
                    );
                    ui.label(
                        RichText::new(format!("Ducad {} Tier — Cloud Active", account.license_tier))
                            .size(11.5)
                            .color(Color32::from_rgb(224, 242, 254)),
                    );
                });
            });

        ui.add_space(8.0);

        // Feature checklist
        ui.label(RichText::new("Fitur Cloud Terhubung:").size(11.0).color(TEXT_SECONDARY));
        ui.label(RichText::new("  ✓ Sinkronisasi Proyek CAD Cloud").size(11.0).color(TEXT_MUTED));
        ui.label(RichText::new("  ✓ Riwayat Versi & Rollback").size(11.0).color(TEXT_MUTED));
        ui.label(RichText::new("  ✓ Standard Parts & Fastener Catalog").size(11.0).color(TEXT_MUTED));

        ui.add_space(12.0);

        // Logout Button
        let logout_btn = ui.add_sized(
            [ui.available_width(), 30.0],
            egui::Button::new(
                RichText::new(format!("{} Keluar dari Akun", ICON_LOGOUT.codepoint))
                    .size(12.5)
                    .color(Color32::from_rgb(248, 113, 113)),
            )
            .fill(Color32::from_rgba_premultiplied(239, 68, 68, 20))
            .stroke(Stroke::new(1.0, Color32::from_rgba_premultiplied(239, 68, 68, 50)))
            .corner_radius(CornerRadius::same(6)),
        );

        if logout_btn.clicked() {
            *event = Some(AccountDrawerEvent::Logout);
        }
    }

    fn render_logged_out(
        ui: &mut Ui,
        event: &mut Option<AccountDrawerEvent>,
    ) {
        ui.label(
            RichText::new("Masuk dengan Akun CMJCode untuk menikmati sinkronisasi proyek CAD cloud dan kolaborasi multi-user.")
                .size(12.0)
                .color(TEXT_SECONDARY),
        );

        ui.add_space(14.0);

        // Tombol Google
        let google_btn = ui.add_sized(
            [ui.available_width(), 34.0],
            egui::Button::new(
                RichText::new("Masuk dengan Google")
                    .size(13.0)
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(Color32::from_rgb(37, 99, 235))
            .corner_radius(CornerRadius::same(6)),
        );

        if google_btn.clicked() {
            *event = Some(AccountDrawerEvent::Login(OAuthProvider::Google));
        }

        ui.add_space(6.0);

        // Tombol GitHub
        let github_btn = ui.add_sized(
            [ui.available_width(), 34.0],
            egui::Button::new(
                RichText::new("Masuk dengan GitHub")
                    .size(13.0)
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(BG_CARD_DARK)
            .stroke(Stroke::new(1.0, BORDER_SUBTLE))
            .corner_radius(CornerRadius::same(6)),
        );

        if github_btn.clicked() {
            *event = Some(AccountDrawerEvent::Login(OAuthProvider::GitHub));
        }
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
                    .color(TEXT_MUTED),
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
