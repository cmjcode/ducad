//! Panel Popup & Pengaturan Akun DUCAD / Cloud Sync untuk DUCAD.

use ducad_cloud::{AuthStatus, DucadAccount, FileList, OAuthProvider};
use ducad_core::{Feature, LicenseTier};
use egui::{
    Align, Align2, Area, Color32, CornerRadius, Frame, Layout, Margin, Order, Pos2, Rect,
    RichText, Sense, Stroke, TextureHandle, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_ADD_A_PHOTO, ICON_APPLE, ICON_CHECK, ICON_CLOSE, ICON_CLOUD, ICON_CLOUD_DOWNLOAD,
    ICON_CLOUD_UPLOAD, ICON_DELETE, ICON_GITHUB, ICON_GOOGLE, ICON_LOCK, ICON_LOGOUT,
    ICON_REFRESH, ICON_SYNC, ICON_WARNING, ICON_WORKSPACE_PREMIUM,
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

/// Keadaan Cloud Sync dan lisensi yang ditampilkan popup Akun. Semua data
/// dimiliki aplikasi; popup hanya menggambar dan mengembalikan event.
#[derive(Clone, Copy, Default)]
pub struct CloudView<'a> {
    /// Tier efektif akun (Free bila tidak login).
    pub tier: LicenseTier,
    /// Daftar berkas terakhir dari server; `None` = belum dimuat.
    pub files: Option<&'a FileList>,
    /// Ada permintaan ke server yang sedang berjalan.
    pub busy: bool,
    /// Pesan hasil operasi terakhir.
    pub status: Option<&'a str>,
    pub status_is_error: bool,
    /// Nama berkas dokumen yang terbuka bila sudah disimpan ke disk;
    /// `None` = tombol unggah dinonaktifkan.
    pub current_document: Option<&'a str>,
    /// Unggahan `name` ditolak karena versi di cloud lebih baru.
    pub conflict: Option<&'a str>,
    /// Id berkas yang menunggu konfirmasi hapus.
    pub confirm_delete: Option<&'a str>,
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
    /// Muat ulang daftar berkas cloud dan tier akun.
    CloudRefresh,
    /// Unggah dokumen yang sedang terbuka.
    CloudUploadCurrent,
    /// Unduh berkas `id` lalu buka.
    CloudOpen(String),
    /// Minta konfirmasi hapus berkas `id`.
    CloudDelete(String),
    CloudDeleteConfirm,
    CloudDeleteCancel,
    /// Konflik: timpa versi cloud dengan versi perangkat ini.
    CloudConflictOverwrite,
    /// Konflik: unggah sebagai salinan bernama baru.
    CloudConflictSaveCopy,
    CloudConflictDismiss,
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
        cloud: CloudView<'_>,
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
                                    Self::render_logged_in(ui, acc, avatar, cloud, &mut event);
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
        cloud: CloudView<'_>,
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
        Self::render_tier(ui, cloud.tier);
        ui.add_space(10.0);
        Self::render_cloud(ui, cloud, event);
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

    /// Kartu tier: nama tier, lalu fitur Pro yang terbuka atau terkunci.
    /// Tidak ada ajakan membeli di sini: build App Store hanya boleh menjual
    /// lewat In-App Purchase (Guideline 3.1.1), dan pembayaran belum ada.
    fn render_tier(ui: &mut Ui, tier: LicenseTier) {
        let pro = tier == LicenseTier::Pro;
        let accent = if pro { ACCENT_GREEN } else { ACCENT_BLUE };
        // Tint dibuat dengan `gamma_multiply` dari warna opak (hasilnya
        // premultiplied yang benar). Jangan pakai `from_rgba_premultiplied`
        // dengan RGB > alpha: egui menganggapnya aditif sehingga badge
        // tampil sebagai blok menyala.
        Frame::new()
            .fill(accent.gamma_multiply(0.14))
            .corner_radius(CornerRadius::same(6))
            .stroke(Stroke::new(1.0, accent.gamma_multiply(0.45)))
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(format!("{}  DUCAD {}", ICON_WORKSPACE_PREMIUM.codepoint, tier.as_str()))
                        .size(12.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.add_space(2.0);
                for feature in Feature::ALL {
                    let (icon, color) = if tier.allows(feature) {
                        (ICON_CHECK.codepoint, ACCENT_GREEN)
                    } else {
                        (ICON_LOCK.codepoint, TEXT_SECONDARY)
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon).size(11.0).color(color));
                        ui.label(RichText::new(feature.label()).size(10.5).color(TEXT_PRIMARY));
                    });
                }
                if !pro {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Fitur bertanda gembok tersedia di DUCAD Pro.")
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );
                }
            });
    }

    /// Bagian Cloud Sync: kuota, daftar berkas, unggah dokumen aktif, dan
    /// penyelesaian konflik.
    fn render_cloud(ui: &mut Ui, cloud: CloudView<'_>, event: &mut Option<AccountDrawerEvent>) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{}  Cloud Sync", ICON_CLOUD.codepoint))
                    .size(12.0)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let refresh = ui.add_enabled(
                    !cloud.busy,
                    egui::Button::new(RichText::new(ICON_REFRESH.codepoint).size(13.0).color(TEXT_SECONDARY))
                        .frame(false),
                );
                if refresh.on_hover_text("Muat ulang").clicked() {
                    *event = Some(AccountDrawerEvent::CloudRefresh);
                }
                if cloud.busy {
                    ui.label(RichText::new(ICON_SYNC.codepoint).size(12.0).color(ACCENT_BLUE));
                }
            });
        });

        if let Some(list) = cloud.files {
            ui.label(
                RichText::new(format!(
                    "{} dari {} - {} / {} berkas",
                    human_bytes(list.used_bytes),
                    human_bytes(list.quota_bytes),
                    list.files.len(),
                    list.max_files
                ))
                .size(10.5)
                .color(TEXT_SECONDARY),
            );
            let frac = if list.quota_bytes == 0 {
                0.0
            } else {
                (list.used_bytes as f32 / list.quota_bytes as f32).clamp(0.0, 1.0)
            };
            ui.add(egui::ProgressBar::new(frac).desired_height(4.0));
        }

        ui.add_space(6.0);
        let upload_label = match cloud.current_document {
            Some(name) => format!("{}  Unggah {}", ICON_CLOUD_UPLOAD.codepoint, name),
            None => format!("{}  Simpan dokumen dulu untuk mengunggah", ICON_CLOUD_UPLOAD.codepoint),
        };
        let upload = ui.add_enabled(
            cloud.current_document.is_some() && !cloud.busy,
            egui::Button::new(RichText::new(upload_label).size(11.5).color(TEXT_PRIMARY))
                .truncate()
                .fill(BG_CARD_DARK)
                .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                .corner_radius(CornerRadius::same(6))
                .min_size(Vec2::new(ui.available_width(), 26.0)),
        );
        if upload.clicked() {
            *event = Some(AccountDrawerEvent::CloudUploadCurrent);
        }

        if let Some(name) = cloud.conflict {
            ui.add_space(6.0);
            Frame::new()
                .fill(ACCENT_RED.gamma_multiply(0.10))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(format!(
                            "{}  {} di cloud diubah dari perangkat lain.",
                            ICON_WARNING.codepoint, name
                        ))
                        .size(11.0)
                        .color(TEXT_PRIMARY),
                    );
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button("Timpa versi cloud").clicked() {
                            *event = Some(AccountDrawerEvent::CloudConflictOverwrite);
                        }
                        if ui.small_button("Simpan sebagai salinan").clicked() {
                            *event = Some(AccountDrawerEvent::CloudConflictSaveCopy);
                        }
                        if ui.small_button("Batal").clicked() {
                            *event = Some(AccountDrawerEvent::CloudConflictDismiss);
                        }
                    });
                });
        }

        if let Some(msg) = cloud.status {
            ui.add_space(4.0);
            let color = if cloud.status_is_error { ACCENT_RED } else { TEXT_SECONDARY };
            ui.label(RichText::new(msg).size(10.5).color(color));
        }

        let Some(list) = cloud.files else {
            return;
        };
        ui.add_space(6.0);
        if list.files.is_empty() {
            ui.label(
                RichText::new("Belum ada berkas di cloud.")
                    .size(10.5)
                    .color(TEXT_SECONDARY),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("ducad_cloud_files")
            .max_height(170.0)
            .show(ui, |ui| {
                for file in &list.files {
                    let confirming = cloud.confirm_delete == Some(file.id.as_str());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 56.0).max(80.0));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&file.name).size(11.0).color(TEXT_PRIMARY),
                                )
                                .truncate(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} - {}",
                                    human_bytes(file.size),
                                    short_timestamp(&file.updated_at)
                                ))
                                .size(9.5)
                                .color(TEXT_SECONDARY),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if confirming {
                                return;
                            }
                            let del = ui.add_enabled(
                                !cloud.busy,
                                egui::Button::new(RichText::new(ICON_DELETE.codepoint).size(13.0).color(ACCENT_RED))
                                    .frame(false),
                            );
                            if del.on_hover_text("Hapus dari cloud").clicked() {
                                *event = Some(AccountDrawerEvent::CloudDelete(file.id.clone()));
                            }
                            let open = ui.add_enabled(
                                !cloud.busy,
                                egui::Button::new(RichText::new(ICON_CLOUD_DOWNLOAD.codepoint).size(13.0).color(ACCENT_BLUE))
                                    .frame(false),
                            );
                            if open.on_hover_text("Unduh dan buka").clicked() {
                                *event = Some(AccountDrawerEvent::CloudOpen(file.id.clone()));
                            }
                        });
                    });
                    if confirming {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Hapus permanen?").size(10.5).color(ACCENT_RED));
                            if ui.small_button("Hapus").clicked() {
                                *event = Some(AccountDrawerEvent::CloudDeleteConfirm);
                            }
                            if ui.small_button("Batal").clicked() {
                                *event = Some(AccountDrawerEvent::CloudDeleteCancel);
                            }
                        });
                    }
                    ui.add_space(2.0);
                }
            });
    }

    fn render_logged_out(ui: &mut Ui, event: &mut Option<AccountDrawerEvent>) {
        ui.label(
            RichText::new("Masuk untuk menyinkronkan berkas .ducad dan preferensi antar perangkat (Mac, iPad, Windows, Linux) dan memakai fitur DUCAD Pro bila akun Anda Pro.")
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

/// Ukuran ringkas: `512 B`, `1.5 KB`, `12.3 MB`, `5.0 GB`.
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64 / 1024.0;
    let mut unit = 0;
    while v >= 1024.0 && unit + 1 < UNITS.len() {
        v /= 1024.0;
        unit += 1;
    }
    format!("{v:.1} {}", UNITS[unit])
}

/// `2026-10-20T10:00:00Z` → `2026-10-20 10:00` (UTC, apa adanya). Masukan
/// yang tidak dikenali dikembalikan utuh.
pub fn short_timestamp(rfc3339: &str) -> String {
    match (rfc3339.get(0..10), rfc3339.get(11..16)) {
        (Some(date), Some(time)) if rfc3339.as_bytes().get(10) == Some(&b'T') => {
            format!("{date} {time} UTC")
        }
        _ => rfc3339.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_bytes_picks_unit() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1536), "1.5 KB");
        assert_eq!(human_bytes(50 * 1024 * 1024), "50.0 MB");
        assert_eq!(human_bytes(5 * 1024 * 1024 * 1024), "5.0 GB");
    }

    #[test]
    fn short_timestamp_trims_seconds_and_keeps_unknown_input() {
        assert_eq!(short_timestamp("2026-10-20T10:05:09Z"), "2026-10-20 10:05 UTC");
        assert_eq!(short_timestamp("kemarin"), "kemarin");
    }
}
